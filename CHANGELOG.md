# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.10](https://github.com/abosnjakovic/oversee/compare/v0.3.9...v0.3.10) - 2026-09-09

### Added

- *(app)* search categories only with a colon prefix
- *(ui)* mark dev processes with a category glyph
- *(app)* group dev processes at the top under the COMMAND sort
- *(process)* carry a dev-tool category on ProcessInfo
- *(category)* classify processes as dev tools from argv

### Fixed

- *(category)* read only the tool, not its arguments
- clear the mechanical and defect-bearing clippy findings
- *(process)* key the port cache by process start time
- *(process)* cache lsof ports so they stop flickering

### Other

- rename SortMode::Name to SortMode::Command
- plan the dev process tier implementation
- spec the dev process tier
- switch tests to nextest and add watch loops
- deny pedantic, nursery and panic-prone clippy lints
- clear the remaining clippy findings
- dedupe navigation arms and drop dead memory API
- Update oversee formula to 0.3.9

## [0.3.9](https://github.com/abosnjakovic/oversee/compare/v0.3.8...v0.3.9) - 2026-08-15

### Fixed

- clear terminal on resize so shrink+grow can't leave stale rows ([#17](https://github.com/abosnjakovic/oversee/pull/17))

### Other

- Update oversee formula to 0.3.8

## [0.3.8](https://github.com/abosnjakovic/oversee/compare/v0.3.7...v0.3.8) - 2026-08-14

### Added

- replace timeline with htop-style bar meters (b toggles placement)
- eighth-block vertical bar helper
- heavy/light-rule horizontal bar helper

### Fixed

- drop stale +/- timeline keys from footer help

### Other

- rustfmt the help colour key lines
- embed help popup screenshot in README
- regenerate README and vhs assets for bar meter UI
- trim ratatui features, restore terminal in panic hook
- remove vertical bar mode, horizontal only
- rewrite help for bar meters + colour key
- implementation plan for htop bar meters
- spec for htop-style bar meters replacing timeline
- Update oversee formula to 0.3.7

## [0.3.7](https://github.com/abosnjakovic/oversee/compare/v0.3.6...v0.3.7) - 2026-08-08

### Fixed

- stop fabricating per-core GPU utilisation

### Other

- Update oversee formula to 0.3.6

## [0.3.6](https://github.com/abosnjakovic/oversee/compare/v0.3.5...v0.3.6) - 2026-08-08

### Added

- *(ui)* toggle timeline between overview, per-core CPU/GPU and memory

### Fixed

- stop fabricating per-process GPU usage ([#12](https://github.com/abosnjakovic/oversee/pull/12))

### Other

- add timeline screenshots and refresh README
- Update oversee formula to 0.3.5

## [0.3.5](https://github.com/abosnjakovic/oversee/compare/v0.3.4...v0.3.5) - 2026-06-23

### Other

- Move per-core CPU/GPU into horizontal header lines ([#11](https://github.com/abosnjakovic/oversee/pull/11))
- Update oversee formula to 0.3.4

## [0.3.4](https://github.com/abosnjakovic/oversee/compare/v0.3.3...v0.3.4) - 2026-06-17

### Fixed

- collapse duplicate local ports in process table ([#9](https://github.com/abosnjakovic/oversee/pull/9))

### Other

- Update oversee formula to 0.3.3

## [0.3.3](https://github.com/abosnjakovic/oversee/compare/v0.3.2...v0.3.3) - 2026-05-11

### Other

- collapse three-workflow release flow into one button ([#8](https://github.com/abosnjakovic/oversee/pull/8))
- Update oversee formula to 0.3.2

## [0.3.2](https://github.com/abosnjakovic/oversee/compare/v0.3.1...v0.3.2) - 2026-05-11

### Other

- cut idle CPU from 5-15% to ~2% ([#6](https://github.com/abosnjakovic/oversee/pull/6))
- Update oversee formula to 0.3.1

## [0.3.1](https://github.com/abosnjakovic/oversee/compare/v0.3.0...v0.3.1) - 2026-05-07

### Other

- migrate release flow to release-plz with manual triggers ([#4](https://github.com/abosnjakovic/oversee/pull/4))
- Replace image in README with new asset
- Update oversee formula to 0.3.0
