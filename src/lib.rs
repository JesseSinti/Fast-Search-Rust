use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use ignore::WalkState;
use memchr::{memchr, memrchr};
use memmap2::Mmap;
use thiserror::Error;
use std::{
    collections::HashSet,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicUsize,AtomicBool, Ordering},
};

#[derive(Error, Debug)]
pub enum SearchError {
    #[error(" IO error occurred: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to map file: {0}")]
    MapError(String),

    #[error("Invalid regex or pattern")]
    PatternError,
}

#[derive(Debug, Clone)]
pub enum SearchResult {
    // How you pass content for text matching to the egui
    ContentMatch {
        path: PathBuf,
        line_number: usize,
        line_text: String,
    },
    // How you pass content for file matching to the egui
    FileNameMatch {
        path: PathBuf,
    },

    ProgressUpdate(usize),
}

// Fields for filtering by and knowing what to look for
pub struct SearchOptions {
    pub root: String,
    pub text_query: Option<String>,
    pub file_query: Option<String>,
    pub ignore_case: bool,
    pub max_depth: usize,
    pub file_types: Option<String>,
}

// Provides a search engine for the matchers and a set of strings for acceptable files
struct SearchConfig {
    text_matcher: Option<AhoCorasick>,
    file_matcher: Option<AhoCorasick>,
    allowed_exts: Option<HashSet<String>>,
}

pub fn run_search(options: SearchOptions, tx: std::sync::mpsc::Sender<SearchResult>, thread_token: Arc<AtomicBool>) {
    // collects the text from SearchOptions and attaches its engine for matching
    let text_matcher = options.text_query.map(|t| {
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(options.ignore_case)
            .build([t])
            .expect("Failed to build text matcher")
    });

    // collects the file name and attaches its engine for matching
    let file_matcher = options.file_query.map(|f| {
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(options.ignore_case)
            .build([f])
            .expect("Failed to build file matcher")
    });

    // collects all file_types and separates them for filtering during actual searching
    let allowed_exts = options.file_types.map(|s| {
        s.split(',').map(|ext| ext.trim().to_lowercase()).collect::<HashSet<_>>()
    });

    // passes the data to a thread
    let config = Arc::new(SearchConfig {
        text_matcher,
        file_matcher,
        allowed_exts,
    });

    
    // Sets up walking through directories starting from the farthest entered
    let mut walker = ignore::WalkBuilder::new(&options.root)
    .max_depth(Some(options.max_depth))
    .hidden(false)
    .git_ignore(true)
    .build_parallel();

    if cfg!(windows) {
        walker = ignore::WalkBuilder::new(&options.root)
        .max_depth(Some(options.max_depth))
        .hidden(true)
        .git_ignore(true)
        // allows multiple to run by splitting them across threads
        .build_parallel();
    } 
    
    let scanned_count = Arc::new(AtomicUsize::new(0));

    // Begins walking through directories
    walker.run(|| {
        let conf = Arc::clone(&config);
        let tx = &tx;
        let count = Arc::clone(&scanned_count);
        let cancel_status = &thread_token;

        // files/directories data being accessed
        Box::new(move |result| {
            let current_val = count.fetch_add(1, Ordering::Relaxed);
            if (current_val + 1) % 50 == 0 {
                let _ = tx.send(SearchResult::ProgressUpdate(50));
            }
            if cancel_status.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }

            // handles issues with permissions blocking entry
            let entry = match result {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };

            // Skips over most files with permission issues/massive sizes
            if entry.depth() > 0 && !is_important(&entry) {
                return WalkState::Skip;
            }

            // Sets the path reference and file name we will use later
            let path = entry.path().to_path_buf();
            let file_name_str = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();

            let mut file_name_match = false;

            // if the File name field has a value it'll come back as true so this knows to search for the inputted file name
            if let Some(ref fm) = conf.file_matcher {
                // uses the AhoCorasick match function to confirm matches
                if fm.is_match(file_name_str.as_ref()) {
                    file_name_match = true;
                    // Sends that data to the egui
                    let _ = tx.send(SearchResult::FileNameMatch { path: path.clone() });
                }
            } else {
                file_name_match = true;
            }

            // If the Text field has a value it'll come back as true and will begin the search
            if let Some(ref tm) = conf.text_matcher {
                if file_name_match && entry.file_type().map_or(false, |ft| ft.is_file()) {
                    let matches_ext = conf.allowed_exts.as_ref().map_or(true, |exts| {
                        path.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| exts.contains(&e.to_lowercase()))
                            .unwrap_or(false)
                    });

                    if matches_ext {
                        if let Ok(file) = File::open(&path) {
                            if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                                if memchr(0, &mmap[..1024.min(mmap.len())]).is_none() {
                                    if let Err(e) = process_file_content(&path, &mmap, tm, &tx) {
                                        eprintln!("Error processing {}: {}", path.display(), e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            WalkState::Continue
        })
    });
}

fn is_important(entry: &ignore::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        "Windows" | "Program Files" | "Program Files (x86)" | "AppData" | "Temp" | ".git" | "node_modules"
    )
}

fn process_file_content(path: &Path, mmap: &[u8], ac: &AhoCorasick, tx: &std::sync::mpsc::Sender<SearchResult>) -> Result<(), SearchError> {
    let mut last_counted_pos = 0;
    let mut current_line_number = 1;

    for mat in ac.find_iter(mmap) {
        let match_start = mat.start();
        current_line_number += bytecount::count(&mmap[last_counted_pos..match_start], b'\n');
        last_counted_pos = match_start;

        let line_start = memrchr(b'\n', &mmap[..match_start]).map(|p| p + 1).unwrap_or(0);
        let line_end = memchr(b'\n', &mmap[match_start..]).map(|p| match_start + p).unwrap_or(mmap.len());

        let line_bytes = &mmap[line_start..line_end];
        let line_text = String::from_utf8_lossy(if line_bytes.ends_with(b"\r") {
            &line_bytes[..line_bytes.len() - 1]
        } else {
            line_bytes
        }).into_owned();

        let _ = tx.send(SearchResult::ContentMatch {
            path: path.to_path_buf(),
            line_number: current_line_number,
            line_text,
        });
    }
    Ok(())
}