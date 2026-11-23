//! The native dialog a person answers before an agent edits or acts (nu9ve-mcp-v1 §3).
//! The model cannot answer it: it is the system's own window, outside the MCP channel.

/// Debug builds only: tests answer `yes` or `no` without a dialog. Release always asks.
pub const QA_ENV: &str = "NU9VE_QA_MCP_CONFIRM";

/// Ask the person. `false` when they say no, close the dialog, or no dialog can be shown.
pub fn confirm(title: &str, question: &str) -> bool {
    #[cfg(debug_assertions)]
    if let Ok(answer) = std::env::var(QA_ENV) {
        return answer == "yes";
    }
    let text = format!(
        "Un asistente conectado por MCP pide:\n\n{question}\n\n¿Permitir? (Si no lo pediste tú, elige No.)"
    );
    dialog(title, &text)
}

#[cfg(target_os = "macos")]
fn dialog(title: &str, text: &str) -> bool {
    // Title and question travel as arguments, never inside the script's source.
    let script = "on run argv\n\
        display dialog (item 2 of argv) with title (item 1 of argv) \
        buttons {\"No\", \"Permitir\"} default button \"No\" cancel button \"No\" with icon caution\n\
        end run";
    std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script, title, text])
        .output()
        .map(|out| {
            out.status.success() && String::from_utf8_lossy(&out.stdout).contains("Permitir")
        })
        .unwrap_or(false)
}

/// Windows (MessageBox) comes with the pipe client; until then, and elsewhere, no.
#[cfg(not(target_os = "macos"))]
fn dialog(_: &str, _: &str) -> bool {
    false
}
