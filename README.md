
A modern system monitor for macOS, inspired by htop and btop++, built in Rust with a focus on Apple Silicon performance monitoring.
Why? I wanted to view cpu AND gpu cores. I wanted memory pressure and not just used/swap due to how macs work differently here.

<img width="1800" height="1169" alt="image" src="https://github.com/user-attachments/assets/7a4fa4ef-be54-44be-ac41-e94fccfc6891" />


## Features

- **Process Management**: Detailed process list with CPU, GPU, memory usage, and user information
- **Timeline Visualization**: Smooth braille character graphs showing system activity over time
- **Memory Pressure**: Green/Yellow/Red pressure indicators matching Activity Monitor
- **Smart Filtering**: Press `/` to filter processes by name, port or user (vim-style)
- **Vim-style controls**: `j/k` for navigation, `g/G` for top/bottom, `/` for search

### Memory Pressure Algorithm
Implements Apple's memory pressure calculation:
- **Green (Normal)**: 50-100% memory available - efficient RAM usage
- **Yellow (Warning)**: 30-50% memory available - using compression
- **Red (Critical)**: 0-30% memory available - heavy swap usage



## Installation

### Prerequisites
- macOS (Apple Silicon or Intel)
- Rust 1.70+ (install via [rustup](https://rustup.rs/))

### Build from Source
```bash
git clone https://github.com/your-username/oversee.git
cd oversee
cargo build --release
./target/release/oversee
```

### Quick Run
```bash
cargo run --release
```

## Usage

### Basic Controls
- `Space`: Pause/Resume monitoring
- `q` or `ESC`: Quit
- `j/k` or `↑↓`: Navigate process list
- `s`: Cycle through sort modes
- `v`: Toggle GPU visibility
- `+/-`: Adjust timeline scope
- `g/G`: Jump to top/bottom of process list

### Filtering Processes
1. Press `/` to enter filter mode
2. Type to filter by process name or username
3. `Enter` to apply filter, `ESC` to cancel
4. Navigation works within filtered results


## Architecture

```
src/
├── main.rs          # Application entry point
├── app.rs           # Main application state and event handling
├── ui.rs            # Terminal UI rendering and layout
├── cpu.rs           # CPU monitoring and history tracking
├── gpu.rs           # Apple Silicon GPU monitoring  
├── memory.rs        # Memory pressure calculation and monitoring
├── process.rs       # Process enumeration with user resolution
└── tui.rs           # Terminal initialization and cleanup
```

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by [htop](https://htop.dev/) and [btop++](https://github.com/aristocratos/btop)
- Built with [ratatui](https://github.com/tui-rs-revival/ratatui) for terminal UI
- Uses [sysinfo](https://github.com/GuillaumeGomez/sysinfo) for cross-platform system information
