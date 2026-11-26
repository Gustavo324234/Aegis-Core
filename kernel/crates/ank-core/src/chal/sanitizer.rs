use super::ExecutionError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum SanitizerMode {
    #[default]
    Normal,
    InTagPrefix,
    InThink,
    InThought,
    InEndThinkPrefix,
    InEndThoughtPrefix,
    InSpecialToken,
}

/// Stateful per-stream text sanitizer that strips leaked reasoning blocks
/// (`<think>...</think>`, `<thought>...</thought>`) and special control tokens
/// (`<|call|>`, `<|im_end|>`, `<|channel|>`, etc.) across arbitrary TCP/SSE chunk splits.
#[derive(Debug, Default)]
pub(super) struct StreamTextSanitizer {
    mode: SanitizerMode,
    buf: String,
}

impl StreamTextSanitizer {
    pub(super) fn push(&mut self, chunk: &str) -> String {
        let mut out = String::new();
        for ch in chunk.chars() {
            if ch == '\0' || ch == '\u{FFFD}' {
                continue;
            }
            match self.mode {
                SanitizerMode::Normal => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InTagPrefix;
                    } else {
                        out.push(ch);
                    }
                }
                SanitizerMode::InTagPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "<|" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InSpecialToken;
                    } else if lc == "<think>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThink;
                    } else if lc == "<thought>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThought;
                    } else if "<think>".starts_with(&lc)
                        || "<thought>".starts_with(&lc)
                        || "<|".starts_with(&lc)
                    {
                        // keep buffering potential tag prefix
                    } else {
                        out.push_str(&self.buf);
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    }
                }
                SanitizerMode::InThink => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InEndThinkPrefix;
                    }
                }
                SanitizerMode::InEndThinkPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "</think>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else if "</think>".starts_with(&lc) {
                        // keep matching </think>
                    } else if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                    } else {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThink;
                    }
                }
                SanitizerMode::InThought => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InEndThoughtPrefix;
                    }
                }
                SanitizerMode::InEndThoughtPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "</thought>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else if "</thought>".starts_with(&lc) {
                        // keep matching </thought>
                    } else if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                    } else {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThought;
                    }
                }
                SanitizerMode::InSpecialToken => {
                    if ch == '>' && self.buf.ends_with('|') {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else {
                        self.buf.push(ch);
                        // Guard against malformed unclosed `<|` sequence
                        if self.buf.len() > 64 {
                            out.push_str("<|");
                            out.push_str(&self.buf);
                            self.buf.clear();
                            self.mode = SanitizerMode::Normal;
                        }
                    }
                }
            }
        }
        out
    }

    pub(super) fn flush(&mut self) -> String {
        match self.mode {
            SanitizerMode::InTagPrefix => {
                self.mode = SanitizerMode::Normal;
                std::mem::take(&mut self.buf)
            }
            _ => {
                self.buf.clear();
                self.mode = SanitizerMode::Normal;
                String::new()
            }
        }
    }
}

pub(super) fn emit_stream_text_chunk(
    text_tx: &tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    chunk: String,
    emit_to_user: bool,
    assistant_text: &mut String,
    has_streamed_text: &mut bool,
    last_streamed_ends_newline: &mut bool,
    needs_paragraph_break: &mut bool,
) {
    if chunk.is_empty() {
        return;
    }
    if emit_to_user {
        if *needs_paragraph_break
            && *has_streamed_text
            && !*last_streamed_ends_newline
            && !chunk.starts_with('\n')
        {
            let _ = text_tx.send(Ok("\n\n".to_string()));
        }
        *needs_paragraph_break = false;
        *has_streamed_text = true;
        *last_streamed_ends_newline = chunk.ends_with('\n');
        let _ = text_tx.send(Ok(chunk.clone()));
    }
    assistant_text.push_str(&chunk);
}
