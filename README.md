
A modern system monitor for macOS, inspired by htop and btop++, built in Rust with a focus on Apple Silicon performance monitoring.

## Features

### 🚀 **System Monitoring**
- **CPU Timeline**: Real-time CPU usage with btop++-style braille pattern visualization
- **GPU Monitoring**: Apple Silicon GPU core utilization and overall usage
- **Memory Pressure**: Apple-style memory pressure monitoring with color-coded status
- **Process Management**: Detailed process list with CPU, GPU, memory usage, and user information

### 📊 **Apple-Style Interface**
- **Timeline Visualization**: Smooth braille character graphs showing system activity over time
- **Memory Pressure**: Green/Yellow/Red pressure indicators matching Activity Monitor
- **Core Monitoring**: Individual CPU and GPU core utilization display
- **Real-time Updates**: 40 FPS responsive interface with optimized rendering

### 🔍 **Advanced Process Features**
- **Smart Filtering**: Press `/` to filter processes by name or user (vim-style)
- **Real User Information**: Actual usernames including system accounts like `_windowserver`, `root`
- **GPU Usage Estimation**: Per-process GPU usage estimation for Apple Silicon
- **Multiple Sort Modes**: Sort by CPU, memory, name, or PID

### ⌨️ **Keyboard Navigation**
- **Vim-style controls**: `j/k` for navigation, `g/G` for top/bottom, `/` for search
- **Timeline controls**: `+/-` to adjust timeline scope (30s/60s/120s/300s)
- **Process management**: Navigate and monitor processes efficiently
- **Filter mode**: Real-time filtering with `Enter` to apply, `ESC` to cancel

## Screenshots

The interface displays:
- **Top section**: CPU/GPU timeline with core utilization panels
- **Middle section**: Memory pressure with usage bar and detailed statistics  
- **Bottom section**: Filterable process list with comprehensive information

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

### Timeline Scopes
- **30s**: High-resolution short-term view
- **60s**: Standard monitoring window  
- **120s**: Medium-term trend analysis
- **300s**: Long-term system behavior

## Technical Details

### Memory Pressure Algorithm
Implements Apple's memory pressure calculation:
- **Green (Normal)**: 50-100% memory available - efficient RAM usage
- **Yellow (Warning)**: 30-50% memory available - using compression
- **Red (Critical)**: 0-30% memory available - heavy swap usage

### Performance Optimizations
- **Efficient Rendering**: Only updates when data changes
- **Frame Rate Limiting**: 40 FPS for responsive navigation
- **Smart Polling**: Optimized system data collection intervals
- **Memory Management**: Bounded history buffers prevent memory leaks

### Apple Silicon Features
- **GPU Core Monitoring**: Individual GPU core utilization tracking
- **Unified Memory**: Proper handling of Apple's unified memory architecture
- **System Account Detection**: Full macOS user account support including service accounts

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
