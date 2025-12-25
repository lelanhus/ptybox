//! Static HTML trace viewer generator.
//!
//! Generates an interactive HTML page from run artifacts that displays:
//! - Timeline of steps with status
//! - Terminal snapshots for each step
//! - Run metadata and assertion results

use miette::{IntoDiagnostic, Result, WrapErr};
use std::fs;
use std::path::Path;
use tui_use::model::{Color, RunResult, ScreenSnapshot};

/// Load artifacts and generate an HTML trace viewer.
pub fn generate_trace(artifacts_dir: &Path, output_path: &Path) -> Result<()> {
    // Load run.json
    let run_path = artifacts_dir.join("run.json");
    let run_content = fs::read_to_string(&run_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", run_path.display()))?;
    let run_result: RunResult = serde_json::from_str(&run_content)
        .into_diagnostic()
        .wrap_err("failed to parse run.json")?;

    // Load snapshots
    let snapshots_dir = artifacts_dir.join("snapshots");
    let snapshots = load_snapshots(&snapshots_dir)?;

    // Load transcript
    let transcript_path = artifacts_dir.join("transcript.log");
    let transcript = fs::read_to_string(&transcript_path).unwrap_or_default();

    // Generate HTML
    let html = render_html(&run_result, &snapshots, &transcript);

    // Write output
    fs::write(output_path, html)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write {}", output_path.display()))?;

    Ok(())
}

fn load_snapshots(snapshots_dir: &Path) -> Result<Vec<ScreenSnapshot>> {
    let mut snapshots = Vec::new();

    if !snapshots_dir.exists() {
        return Ok(snapshots);
    }

    let mut entries: Vec<_> = fs::read_dir(snapshots_dir)
        .into_diagnostic()
        .wrap_err("failed to read snapshots directory")?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let content = fs::read_to_string(entry.path())
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to read {}", entry.path().display()))?;
        let snapshot: ScreenSnapshot = serde_json::from_str(&content)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to parse {}", entry.path().display()))?;
        snapshots.push(snapshot);
    }

    Ok(snapshots)
}

fn render_html(run_result: &RunResult, snapshots: &[ScreenSnapshot], transcript: &str) -> String {
    let steps_json = serde_json::to_string(&run_result.steps).unwrap_or_else(|_| "[]".to_string());
    let snapshots_json = serde_json::to_string(snapshots).unwrap_or_else(|_| "[]".to_string());
    let run_json = serde_json::to_string(run_result).unwrap_or_else(|_| "{}".to_string());
    let transcript_escaped = html_escape(transcript);

    let status_class = match run_result.status {
        tui_use::model::RunStatus::Passed => "status-passed",
        tui_use::model::RunStatus::Failed => "status-failed",
        tui_use::model::RunStatus::Errored => "status-errored",
        tui_use::model::RunStatus::Canceled => "status-canceled",
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>tui-use Trace Viewer - {run_id}</title>
    <style>
{CSS}
    </style>
</head>
<body>
    <header>
        <h1>tui-use Trace Viewer</h1>
        <div class="run-info">
            <span class="run-id">Run: {run_id}</span>
            <span class="{status_class}">{status:?}</span>
            <span class="duration">{duration_ms}ms</span>
        </div>
    </header>

    <main>
        <div class="panel left-panel">
            <h2>Steps</h2>
            <div id="steps-list" class="steps-list"></div>
        </div>

        <div class="panel center-panel">
            <h2>Terminal Snapshot</h2>
            <div id="terminal" class="terminal"></div>
        </div>

        <div class="panel right-panel">
            <h2>Details</h2>
            <div id="details" class="details"></div>
            <h2>Transcript</h2>
            <pre id="transcript" class="transcript">{transcript_escaped}</pre>
        </div>
    </main>

    <footer>
        <span id="current-step">Select a step to view</span>
        <div class="nav-controls">
            <button id="prev-btn" title="Previous snapshot">&larr;</button>
            <span id="snapshot-index">0 / 0</span>
            <button id="next-btn" title="Next snapshot">&rarr;</button>
        </div>
    </footer>

    <script>
const STEPS = {steps_json};
const SNAPSHOTS = {snapshots_json};
const RUN = {run_json};

{JS}
    </script>
</body>
</html>
"#,
        run_id = run_result.run_id,
        status = run_result.status,
        status_class = status_class,
        duration_ms = run_result.ended_at_ms.saturating_sub(run_result.started_at_ms),
        transcript_escaped = transcript_escaped,
        steps_json = steps_json,
        snapshots_json = snapshots_json,
        run_json = run_json,
        CSS = CSS,
        JS = JS,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Convert a Color to CSS color string.
#[allow(dead_code)]
fn color_to_css(color: &Color) -> String {
    match color {
        Color::Default => String::new(),
        Color::Ansi16(n) => {
            // Standard 16 ANSI colors
            let colors = [
                "#000000", "#cd0000", "#00cd00", "#cdcd00", "#0000ee", "#cd00cd", "#00cdcd",
                "#e5e5e5", "#7f7f7f", "#ff0000", "#00ff00", "#ffff00", "#5c5cff", "#ff00ff",
                "#00ffff", "#ffffff",
            ];
            (*colors.get(*n as usize).unwrap_or(&"")).to_string()
        }
        Color::Ansi256(n) => {
            let n = *n as usize;
            if n < 16 {
                // Safe: n is checked to be < 16, which fits in u8
                #[allow(clippy::cast_possible_truncation)]
                return color_to_css(&Color::Ansi16(n as u8));
            }
            if n < 232 {
                // 6x6x6 color cube
                let idx = n - 16;
                let r = (idx / 36) * 51;
                let g = ((idx / 6) % 6) * 51;
                let b = (idx % 6) * 51;
                return format!("#{r:02x}{g:02x}{b:02x}");
            }
            // Grayscale
            let gray = 8 + (n - 232) * 10;
            format!("#{gray:02x}{gray:02x}{gray:02x}")
        }
        Color::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

const CSS: &str = r"
:root {
    --bg-primary: #1a1a2e;
    --bg-secondary: #16213e;
    --bg-terminal: #0f0f23;
    --text-primary: #eee;
    --text-secondary: #aaa;
    --border-color: #333;
    --status-passed: #4caf50;
    --status-failed: #f44336;
    --status-errored: #ff9800;
    --status-canceled: #9e9e9e;
    --accent: #00bcd4;
}

* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
    height: 100vh;
    display: flex;
    flex-direction: column;
}

header {
    padding: 1rem;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    display: flex;
    justify-content: space-between;
    align-items: center;
}

header h1 {
    font-size: 1.2rem;
    font-weight: 500;
}

.run-info {
    display: flex;
    gap: 1rem;
    align-items: center;
}

.run-id {
    color: var(--text-secondary);
    font-family: monospace;
    font-size: 0.85rem;
}

.status-passed { color: var(--status-passed); font-weight: 600; }
.status-failed { color: var(--status-failed); font-weight: 600; }
.status-errored { color: var(--status-errored); font-weight: 600; }
.status-canceled { color: var(--status-canceled); font-weight: 600; }

.duration {
    color: var(--text-secondary);
    font-size: 0.85rem;
}

main {
    flex: 1;
    display: flex;
    overflow: hidden;
}

.panel {
    border-right: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.panel h2 {
    padding: 0.75rem 1rem;
    font-size: 0.9rem;
    font-weight: 600;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
}

.left-panel {
    width: 280px;
    min-width: 200px;
}

.center-panel {
    flex: 1;
    min-width: 400px;
}

.right-panel {
    width: 350px;
    min-width: 250px;
    border-right: none;
}

.steps-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
}

.step-item {
    padding: 0.75rem;
    margin-bottom: 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    background: var(--bg-secondary);
    border: 1px solid transparent;
    transition: all 0.15s ease;
}

.step-item:hover {
    border-color: var(--accent);
}

.step-item.selected {
    border-color: var(--accent);
    background: rgba(0, 188, 212, 0.1);
}

.step-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
}

.step-name {
    font-size: 0.85rem;
    font-weight: 500;
}

.step-status {
    font-size: 0.75rem;
    padding: 0.15rem 0.5rem;
    border-radius: 3px;
}

.step-status.passed { background: rgba(76, 175, 80, 0.2); color: var(--status-passed); }
.step-status.failed { background: rgba(244, 67, 54, 0.2); color: var(--status-failed); }
.step-status.errored { background: rgba(255, 152, 0, 0.2); color: var(--status-errored); }
.step-status.skipped { background: rgba(158, 158, 158, 0.2); color: var(--status-canceled); }

.step-meta {
    font-size: 0.75rem;
    color: var(--text-secondary);
}

.terminal {
    flex: 1;
    overflow: auto;
    padding: 1rem;
    background: var(--bg-terminal);
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 13px;
    line-height: 1.4;
}

.terminal-line {
    white-space: pre;
    min-height: 1.4em;
}

.terminal-cursor {
    background: var(--accent);
    animation: blink 1s step-end infinite;
}

@keyframes blink {
    50% { opacity: 0; }
}

.details {
    padding: 1rem;
    overflow-y: auto;
    max-height: 300px;
}

.detail-section {
    margin-bottom: 1rem;
}

.detail-section h3 {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
}

.assertion-item {
    padding: 0.5rem;
    margin-bottom: 0.25rem;
    border-radius: 3px;
    font-size: 0.8rem;
    background: var(--bg-secondary);
}

.assertion-item.passed { border-left: 3px solid var(--status-passed); }
.assertion-item.failed { border-left: 3px solid var(--status-failed); }

.transcript {
    flex: 1;
    overflow: auto;
    padding: 1rem;
    background: var(--bg-terminal);
    font-family: monospace;
    font-size: 0.75rem;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 200px;
}

footer {
    padding: 0.75rem 1rem;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border-color);
    display: flex;
    justify-content: space-between;
    align-items: center;
}

#current-step {
    color: var(--text-secondary);
    font-size: 0.85rem;
}

.nav-controls {
    display: flex;
    gap: 0.5rem;
    align-items: center;
}

.nav-controls button {
    padding: 0.5rem 1rem;
    border: 1px solid var(--border-color);
    background: var(--bg-secondary);
    color: var(--text-primary);
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
}

.nav-controls button:hover {
    border-color: var(--accent);
}

.nav-controls button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

#snapshot-index {
    font-size: 0.85rem;
    color: var(--text-secondary);
    min-width: 60px;
    text-align: center;
}

/* Cell styling */
.cell-bold { font-weight: bold; }
.cell-italic { font-style: italic; }
.cell-underline { text-decoration: underline; }
.cell-inverse { filter: invert(1); }
";

const JS: &str = r#"
let currentSnapshotIndex = 0;

function init() {
    renderStepsList();
    updateNavigation();
    if (SNAPSHOTS.length > 0) {
        renderSnapshot(0);
    }
    if (STEPS && STEPS.length > 0) {
        selectStep(0);
    }

    document.getElementById('prev-btn').onclick = () => navigate(-1);
    document.getElementById('next-btn').onclick = () => navigate(1);
    document.addEventListener('keydown', handleKeydown);
}

function renderStepsList() {
    const container = document.getElementById('steps-list');
    if (!STEPS || STEPS.length === 0) {
        container.innerHTML = '<div class="step-item">No steps recorded</div>';
        return;
    }

    container.innerHTML = STEPS.map((step, i) => {
        const duration = step.ended_at_ms - step.started_at_ms;
        const statusClass = step.status.toLowerCase();
        return `
            <div class="step-item" data-index="${i}" onclick="selectStep(${i})">
                <div class="step-header">
                    <span class="step-name">${escapeHtml(step.name || 'Step ' + (i + 1))}</span>
                    <span class="step-status ${statusClass}">${step.status}</span>
                </div>
                <div class="step-meta">
                    ${step.action ? step.action.type : 'unknown'} &middot; ${duration}ms
                </div>
            </div>
        `;
    }).join('');
}

function selectStep(index) {
    // Update selection visual
    document.querySelectorAll('.step-item').forEach((el, i) => {
        el.classList.toggle('selected', i === index);
    });

    // Update details panel
    const step = STEPS[index];
    renderDetails(step);

    // Update footer
    document.getElementById('current-step').textContent =
        `Step ${index + 1} of ${STEPS.length}: ${step.name || 'Unnamed'}`;

    // Find matching snapshot (simplified: use index)
    if (index < SNAPSHOTS.length) {
        currentSnapshotIndex = index;
        renderSnapshot(index);
        updateNavigation();
    }
}

function renderDetails(step) {
    const container = document.getElementById('details');
    if (!step) {
        container.innerHTML = '<div class="detail-section">No step selected</div>';
        return;
    }

    let html = '';

    // Action info
    if (step.action) {
        html += `
            <div class="detail-section">
                <h3>Action</h3>
                <div style="font-family: monospace; font-size: 0.8rem; padding: 0.5rem; background: var(--bg-terminal); border-radius: 4px;">
                    <strong>${escapeHtml(step.action.type)}</strong>
                    ${step.action.payload ? '<br>' + escapeHtml(JSON.stringify(step.action.payload, null, 2)) : ''}
                </div>
            </div>
        `;
    }

    // Assertions
    if (step.assertions && step.assertions.length > 0) {
        html += `
            <div class="detail-section">
                <h3>Assertions (${step.assertions.length})</h3>
                ${step.assertions.map(a => `
                    <div class="assertion-item ${a.passed ? 'passed' : 'failed'}">
                        <strong>${escapeHtml(a.type)}</strong>
                        ${a.message ? '<br>' + escapeHtml(a.message) : ''}
                    </div>
                `).join('')}
            </div>
        `;
    }

    // Error
    if (step.error) {
        html += `
            <div class="detail-section">
                <h3>Error</h3>
                <div style="color: var(--status-failed); font-family: monospace; font-size: 0.8rem;">
                    <strong>${escapeHtml(step.error.code)}</strong>: ${escapeHtml(step.error.message)}
                </div>
            </div>
        `;
    }

    container.innerHTML = html || '<div class="detail-section">No details available</div>';
}

function renderSnapshot(index) {
    const container = document.getElementById('terminal');

    if (index < 0 || index >= SNAPSHOTS.length) {
        container.innerHTML = '<div style="color: var(--text-secondary); padding: 2rem; text-align: center;">No snapshot available</div>';
        return;
    }

    const snapshot = SNAPSHOTS[index];

    // Render lines with optional cell styling
    let html = '';
    if (snapshot.cells && snapshot.cells.length > 0) {
        // Rich cell rendering
        snapshot.cells.forEach((row, rowIdx) => {
            html += '<div class="terminal-line">';
            row.forEach((cell, colIdx) => {
                const styles = [];
                const classes = [];

                if (cell.style) {
                    if (cell.style.fg && cell.style.fg !== 'default') {
                        const color = colorToCss(cell.style.fg);
                        if (color) styles.push(`color: ${color}`);
                    }
                    if (cell.style.bg && cell.style.bg !== 'default') {
                        const color = colorToCss(cell.style.bg);
                        if (color) styles.push(`background: ${color}`);
                    }
                    if (cell.style.bold) classes.push('cell-bold');
                    if (cell.style.italic) classes.push('cell-italic');
                    if (cell.style.underline) classes.push('cell-underline');
                    if (cell.style.inverse) classes.push('cell-inverse');
                }

                // Cursor indicator
                if (snapshot.cursor && snapshot.cursor.row === rowIdx && snapshot.cursor.col === colIdx && snapshot.cursor.visible) {
                    classes.push('terminal-cursor');
                }

                const ch = escapeHtml(cell.ch || ' ');
                if (styles.length > 0 || classes.length > 0) {
                    html += `<span class="${classes.join(' ')}" style="${styles.join(';')}">${ch}</span>`;
                } else {
                    html += ch;
                }
            });
            html += '</div>';
        });
    } else {
        // Simple line rendering
        snapshot.lines.forEach((line, rowIdx) => {
            html += '<div class="terminal-line">';
            for (let colIdx = 0; colIdx < line.length; colIdx++) {
                const ch = escapeHtml(line[colIdx] || ' ');
                if (snapshot.cursor && snapshot.cursor.row === rowIdx && snapshot.cursor.col === colIdx && snapshot.cursor.visible) {
                    html += `<span class="terminal-cursor">${ch}</span>`;
                } else {
                    html += ch;
                }
            }
            html += '</div>';
        });
    }

    container.innerHTML = html;
    currentSnapshotIndex = index;
}

function colorToCss(color) {
    if (!color || color === 'default') return null;

    // Handle object format: { ansi16: N }, { ansi256: N }, { rgb: {r, g, b} }
    if (typeof color === 'object') {
        if (color.ansi16 !== undefined) {
            const colors = [
                '#000000', '#cd0000', '#00cd00', '#cdcd00', '#0000ee', '#cd00cd', '#00cdcd', '#e5e5e5',
                '#7f7f7f', '#ff0000', '#00ff00', '#ffff00', '#5c5cff', '#ff00ff', '#00ffff', '#ffffff'
            ];
            return colors[color.ansi16] || null;
        }
        if (color.ansi256 !== undefined) {
            const n = color.ansi256;
            if (n < 16) return colorToCss({ ansi16: n });
            if (n < 232) {
                const idx = n - 16;
                const r = Math.floor(idx / 36) * 51;
                const g = Math.floor((idx / 6) % 6) * 51;
                const b = (idx % 6) * 51;
                return `rgb(${r}, ${g}, ${b})`;
            }
            const gray = 8 + (n - 232) * 10;
            return `rgb(${gray}, ${gray}, ${gray})`;
        }
        if (color.rgb !== undefined) {
            return `rgb(${color.rgb.r}, ${color.rgb.g}, ${color.rgb.b})`;
        }
    }
    return null;
}

function navigate(delta) {
    const newIndex = currentSnapshotIndex + delta;
    if (newIndex >= 0 && newIndex < SNAPSHOTS.length) {
        currentSnapshotIndex = newIndex;
        renderSnapshot(newIndex);
        updateNavigation();

        // Also select corresponding step if available
        if (newIndex < (STEPS?.length || 0)) {
            selectStep(newIndex);
        }
    }
}

function updateNavigation() {
    document.getElementById('prev-btn').disabled = currentSnapshotIndex <= 0;
    document.getElementById('next-btn').disabled = currentSnapshotIndex >= SNAPSHOTS.length - 1;
    document.getElementById('snapshot-index').textContent =
        SNAPSHOTS.length > 0 ? `${currentSnapshotIndex + 1} / ${SNAPSHOTS.length}` : '0 / 0';
}

function handleKeydown(e) {
    if (e.key === 'ArrowLeft' || e.key === 'h') navigate(-1);
    if (e.key === 'ArrowRight' || e.key === 'l') navigate(1);
    if (e.key === 'ArrowUp' || e.key === 'k') {
        const steps = STEPS?.length || 0;
        if (steps > 0) {
            const current = Array.from(document.querySelectorAll('.step-item')).findIndex(el => el.classList.contains('selected'));
            if (current > 0) selectStep(current - 1);
        }
    }
    if (e.key === 'ArrowDown' || e.key === 'j') {
        const steps = STEPS?.length || 0;
        if (steps > 0) {
            const current = Array.from(document.querySelectorAll('.step-item')).findIndex(el => el.classList.contains('selected'));
            if (current < steps - 1) selectStep(current + 1);
        }
    }
}

function escapeHtml(str) {
    if (!str) return '';
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

init();
"#;
