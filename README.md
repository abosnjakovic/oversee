A modern system monitor for macOS, inspired by htop and btop++, built in Rust with a focus on Apple Silicon performance monitoring.
Why? I wanted to view cpu AND gpu cores. I wanted memory pressure and not just used/swap due to how macs work differently here.

<img width="6016" height="3384" alt="image" src="https://github.com/user-attachments/assets/f9a3476e-d6f1-423b-91c4-0af32a4d5a2f" />


## Features

- **Process Management**: Detailed process list with CPU, GPU, memory usage, and user information
- **GPU Monitoring**: Real-time GPU utilisation via powermetrics (requires sudo)
- **Timeline Visualization**: Smooth braille character graphs showing system activity over time
- **Memory Pressure**: Green/Yellow/Red pressure indicators matching Activity Monitor
- **Smart Filtering**: Press `/` to filter processes by name, port or user (vim-style)
- **Vim-style controls**: `j/k` for navigation, `g/G` for top/bottom, `/` for search

### Memory Pressure Algorithm
Uses macOS's native memory pressure reporting via `kern.memorystatus_vm_pressure_level`:
- **Green (Normal)**: System has adequate memory and is operating efficiently
- **Yellow (Warning)**: System is under some memory pressure, may be using compression
- **Red (Critical)**: System is under severe memory pressure, performance may be impacted

This matches Activity Monitor's behavior exactly, as both use the same kernel metric.

### GPU Monitoring
GPU utilisation is obtained via macOS `powermetrics` which requires root access. Run oversee with `sudo` for accurate GPU metrics. Without sudo, GPU shows 0%.

### Understanding macOS Memory Management

If you're coming from Windows or Linux, you might be alarmed to see your Mac using 70-80% of its RAM with just a few apps open. Don't panic—this is exactly what macOS is designed to do, and it's actually making your system faster.

Apple follows a fundamental principle: **"Unused RAM is wasted RAM."** Think of it like a master chef's workspace. While a novice might clean and put away every tool after each use, a professional chef keeps frequently used knives, pans, and ingredients within arm's reach. They know that constantly retrieving and storing tools wastes precious time during service.

macOS treats memory the same way. When you close an app, macOS doesn't immediately purge it from memory. Instead, it marks that memory as "purgeable" but keeps the data cached. Launch the app again, and it springs to life instantly because it never truly left. This is why opening Safari for the second time in a day feels instantaneous compared to that first morning launch.

But here's where macOS truly diverges from its competitors: **memory compression**. When memory starts filling up, Windows and Linux traditionally start swapping to disk—a slow process that can make your system feel sluggish. macOS, however, first attempts to compress inactive memory pages, squeezing them down to about half their size while keeping them in RAM. It's like vacuum-packing winter clothes in your closet—same items, less space, and much faster to access than retrieving them from the basement.

This compression happens silently in the background. You might have 16GB of RAM with 12GB "used," but several gigabytes of that could be compressed data that macOS can instantly decompress when needed—orders of magnitude faster than reading from even the fastest SSD.

The **unified buffer cache** is another piece of magic. Unlike traditional systems that maintain separate caches for files and applications, macOS uses a single, intelligent cache that adapts to your usage patterns. Working on a video project? The cache prioritizes your media files. Coding all day? Your source files and development tools get priority. It's constantly learning and adapting.

This is why **Memory Pressure**, not percentage used, is the true indicator of your Mac's memory health. You could be at 90% memory usage with pressure in the green, and your Mac will feel perfectly responsive because most of that "used" memory is just cached data that can be instantly discarded if needed. It's the difference between a library with books on shelves (high usage, low pressure) versus one with books stacked on every surface including the floors (high usage, high pressure).

So when Oversee shows high memory usage but green pressure, your Mac isn't struggling—it's performing optimally, keeping everything you might need at its fingertips, ready to deliver the smooth, responsive experience Mac users expect.


## Installation

From cargo via crates.io
```bash
cargo install oversee
```

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

### Running with GPU Metrics
For accurate GPU utilisation data, run with sudo:
```bash
sudo cargo run --release
# or after building
sudo ./target/release/oversee
```
Without sudo, GPU metrics will show 0% (requires access to powermetrics).

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
