# Fast Search Engine

## Overview
**Fast Search Engine** is a high-performance, multi-threaded desktop application designed for rapid local file and content searching. Built with Rust, it features a modern GUI and leverages advanced search algorithms and memory mapping to quickly scan through massive directories while intelligently skipping unnecessary system files. 

## Key Features
* **Lightning-Fast Matching**: Utilizes the Aho-Corasick algorithm to execute high-speed text and file name pattern matching.
* **Responsive GUI**: Built with the `egui` framework, providing a clean, dark-themed interface that remains responsive even during heavy background processing.
* **Memory-Mapped Reads**: Uses `memmap2` to map file contents directly into memory, dramatically increasing read speeds for content matching.
* **Smart Directory Traversal**: Automatically ignores hidden files, `.gitignore` paths, and skips over heavy system directories like `node_modules`, `.git`, `Windows`, and `Program Files` to save time.
* **Advanced Filtering**: Allows users to narrow down searches by specifying file extensions, toggling case sensitivity, and setting maximum directory depths.
* **Interactive Results**: Click on any search result to open the file directly, or right-click to open its containing folder.
* **Asynchronous Execution**: Searches run on a separate thread with a real-time progress indicator, allowing you to cancel long-running operations at any time without freezing the app.

## Tech Stack
* **Rust**: Core programming language.
* **eframe / egui**: Immediate mode GUI framework for the visual interface.
* **aho-corasick**: String search algorithm for fast multi-pattern matching.
* **ignore**: Fast directory traversal that respects `.gitignore` rules.
* **memmap2**: Memory-mapped file I/O for performance.
* **memchr**: Highly optimized routines for string search primitives.

## Usage
1. **Root Path**: Select or paste the directory path you want to search. You can use the folder icon to browse your system.
2. **Search Text**: Enter the specific string of text you want to find within files.
3. **Search File Name**: Enter a file name (or partial name) to locate specific files.
4. **File Types/Extensions**: Restrict the search to certain file types by listing extensions separated by commas (e.g., `rs, txt, md`).
5. **Advanced Options**: Toggle case sensitivity or limit how deep the search traverses into subdirectories.
6. **Start/Cancel**: Click "Start Search" to begin or press Enter. You can halt the scan mid-way using the "Cancel" button.

## Project Structure
* `src/lib.rs`: Contains the core search engine logic (`SearchOptions`, `run_search`), multi-threading configuration, directory walking rules, and content processing functions.
* `src/main.rs`: Contains the `egui` application state (`FastSearchApp`), UI layout, user input handling, and the result rendering logic.
