# htop-style bar meters (timeline removed)

Date: 2026-08-15
Status: approved

## Context

Supersedes `2026-08-14-btop-mirror-graph-inset-panel-design.md` (built, then
rejected; parked on unapplied branch `btop-mirror-panel`). New direction:
pure htop — the braille history timeline is deleted entirely and replaced by
a section of bar meters showing *current* values only: one bar per CPU core,
one system-wide GPU bar, memory and swap bars. Two placements, toggled at
runtime.

## Bar styles

**Horizontal (default):** heavy/light rule style, no brackets:

```
C0  ━━━━━━━╸··········  43%
GPU ━╸················   8%
MEM ━━━━━━━━━━━╸······  30.0/48.0G
```

- Fill `━`, half-step tail `╸` (2× per-cell resolution), remainder `·` in
  `THEME.grid` faint style.
- Label left (4 cols, `C13 `/`GPU `), value right-aligned after the bar:
  cores and GPU show `NN%`, MEM `used/totalG`, SWP `used/totalG`.
- Fill colour: `get_gradient_color(value)` for cores and GPU; MEM/SWP bar
  colour comes from memory pressure (green→`THEME.mem`, yellow→warn,
  red→crit) — **colour only**, no warning text.
- Bars flow down a 2-column grid: cores first, then GPU (only when GPU
  visible), MEM, SWP (only when `total_swap > 0`).

**Vertical:** equalizer — one column per metric, bars 2 chars wide with 1
space gap, 8 rows tall, eighth-block resolution (`▁▂▃▄▅▆▇█`), followed by a
label row and a value row (percentages):

```
 █           ▂
 █  ▅  ▂     █        ▄
 █  █  █  ▅  █  ▁  ▃  █
 C0 C1 C2 C3 …  GPU MEM SWP
 43 35 30 22     8  62   1
```

Same colour rules as horizontal.

## Layout

```
oversee · load 3.38 3.27 2.64 · 1148 procs · up 20d07h · Apple M4 Pro
─────────────────────────────────────────────────────────────────────
<bar section — height depends on placement and item count>
─────────────────────────────────────────────────────────────────────
processes …
```

- Header gains the three load averages and the CPU brand string; the
  timeline position text is gone.
- Bar section height: horizontal = `ceil(items / 2)` rows; vertical = 10
  rows (8 bar + label + value). Process list keeps `Min(8)`.
- CPU brand via `sysctl -n machdep.cpu.brand_string` (one `Command` call in
  `App::new`, stored as `App::cpu_brand`, fallback `"cpu"`).

## Keys

- `b` — toggle bar placement (`BarLayout::Horizontal` ⇄ `BarLayout::Vertical`)
- Removed (timeline gone): `Tab`/`t` view cycling, `+`/`=`/`-` offset scroll
- Kept: `v` GPU bar hide/show, `space` pause, all process-list keys
- `?` help rewritten: keybinds updated; timeline section deleted; new
  **colour key** section documenting the load gradient thresholds
  (<25 dim, ≥25 yellow-green, ≥50 warn, ≥75 orange, ≥90 critical) and the
  memory pressure colours (green/yellow/red on the MEM and SWP bars).

## App slimming

Bars need current values only — history buffers die:

- Delete: `cpu_core_histories`, `gpu_overall_history`,
  `memory_usage_history`, `memory_pressure_history`, `cpu_average_history`,
  `TimelineView`, `timeline_view`, `timeline_offset`,
  `MAX_TIMELINE_OFFSET`, `get_timeline_position_text`,
  `get_timeline_offset`, `get_cpu_average_history`, `get_cpu_usages`.
- Add: `cpu_core_values: Vec<f32>`, `cpu_average: f32`, `gpu_value: f32`,
  `bar_layout: BarLayout`, `cpu_brand: String`.
- `process_updates` stores the latest values directly; `memory_info` stays
  as-is.

## ui.rs deletions

`render_chart_timeline`, `render_waves`, `Wave`, `core_palette`,
`CORE_HUES`, `pressure_col_palettes`, `interpolate_data`,
`get_history_slice`, `get_display_slice`, `get_braille_bits`,
`get_vertical_line_bits`, `render_cpu_cores_line`, `current_load_one`
(load moves into header), plus the legacy dot-pattern/core-name tests that
only exercise test-local helpers. `render_memory_section` dies too (MEM/SWP
become bars). `trail_tier`/`TRAIL_TIERS` imports drop from ui.rs (theme
keeps the definitions; other consumers unaffected).

## GPU constraint

macOS provides system-wide GPU utilisation only (`gpu.rs` powermetrics
sampler) — one GPU bar. Per-GPU-core bars are impossible without
fabricating data, which this repo has already removed twice.

## Edge cases

- More items than fit the width (vertical mode, narrow terminal): clip
  trailing bars; never wrap.
- `memory_info == None` at startup: MEM/SWP bars render empty with `—`
  value.
- GPU hidden or unavailable: GPU bar omitted (both modes).
- Terminal narrower than one horizontal bar column: single-column grid.

## Testing

- `hori_bar(frac, width)`: exact char count for any width; 0.0 → all `·`;
  1.0 → all `━`; half-step boundary (e.g. 0.43 × 18 cells → 7 full + 1
  half + 10 empty).
- `vert_bar_levels(frac, rows)`: 0.0 → all blank; 1.0 → all `█`; partial
  eighth-block at the boundary row.
- Grid layout: item→(row, col) mapping for the 2-column horizontal flow.
- Manual: run app, toggle `b`, `v`, narrow terminal, help popup.
