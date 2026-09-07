use std::mem;

use helix_core::completion::CompletionProvider;
use helix_lsp::{lsp, LanguageServerId};
use helix_view::handlers::completion::ResponseContext;

pub struct CompletionResponse {
    pub items: CompletionItems,
    pub provider: CompletionProvider,
    pub context: ResponseContext,
}

pub enum CompletionItems {
    Lsp(Vec<lsp::CompletionItem>),
    Other(Vec<CompletionItem>),
}

impl CompletionItems {
    pub fn is_empty(&self) -> bool {
        match self {
            CompletionItems::Lsp(items) => items.is_empty(),
            CompletionItems::Other(items) => items.is_empty(),
        }
    }
}

impl CompletionResponse {
    pub fn take_items(&mut self, dst: &mut Vec<CompletionItem>) {
        match &mut self.items {
            CompletionItems::Lsp(items) => dst.extend(items.drain(..).map(|item| {
                CompletionItem::Lsp(LspCompletionItem {
                    item,
                    provider: match self.provider {
                        CompletionProvider::Lsp(provider) => provider,
                        _ => unreachable!(),
                    },
                    resolved: false,
                    provider_priority: self.context.priority,
                })
            })),
            CompletionItems::Other(items) if dst.is_empty() => mem::swap(dst, items),
            CompletionItems::Other(items) => dst.append(items),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LspCompletionItem {
    pub item: lsp::CompletionItem,
    pub provider: LanguageServerId,
    pub resolved: bool,
    // TODO: we should not be filtering and sorting incomplete completion list
    // according to the spec but vscode does that anyway and most servers (
    // including rust-analyzer) rely on that.. so we can't do that without
    // breaking completions.
    pub provider_priority: i8,
}

impl LspCompletionItem {
    #[inline]
    pub fn filter_text(&self) -> &str {
        self.item
            .filter_text
            .as_ref()
            .unwrap_or(&self.item.label)
            .as_str()
    }
}

#[allow(clippy::large_enum_variant)] // TODO: In a separate PR attempt the `Box<LspCompletionItem>` pattern.
#[derive(Debug, PartialEq, Clone)]
pub enum CompletionItem {
    Lsp(LspCompletionItem),
    Other(helix_core::CompletionItem),
}

impl CompletionItem {
    #[inline]
    pub fn filter_text(&self) -> &str {
        match self {
            CompletionItem::Lsp(item) => item.filter_text(),
            CompletionItem::Other(item) => &item.label,
        }
    }
}

impl PartialEq<CompletionItem> for LspCompletionItem {
    fn eq(&self, other: &CompletionItem) -> bool {
        match other {
            CompletionItem::Lsp(other) => self == other,
            _ => false,
        }
    }
}

impl PartialEq<CompletionItem> for helix_core::CompletionItem {
    fn eq(&self, other: &CompletionItem) -> bool {
        match other {
            CompletionItem::Other(other) => self == other,
            _ => false,
        }
    }
}

impl CompletionItem {
    pub fn provider_priority(&self) -> i8 {
        match self {
            CompletionItem::Lsp(item) => item.provider_priority,
            // sorting path completions after LSP for now
            CompletionItem::Other(_) => 1,
        }
    }

    pub fn provider(&self) -> CompletionProvider {
        match self {
            CompletionItem::Lsp(item) => CompletionProvider::Lsp(item.provider),
            CompletionItem::Other(item) => item.provider,
        }
    }

    pub fn preselect(&self) -> bool {
        match self {
            CompletionItem::Lsp(LspCompletionItem { item, .. }) => item.preselect.unwrap_or(false),
            CompletionItem::Other(_) => false,
        }
    }

    /// Whether this item should rank below code and symbol matches in the
    /// completion menu. Julia's REPL ships ~1200 `\:`-prefixed emoji
    /// completions that language servers return for every backslash prefix
    /// with the same kind and no sort text as real symbols, so they match
    /// by fuzzy accident and crowd out real completions.
    pub fn demote_julia_emoji_below(&self, typed: &str) -> bool {
        demote_julia_emoji_label(self.filter_text(), typed)
    }
}

/// Reports whether a completion label should rank below real matches for
/// the given typed filter text. Labels in Julia's `\:` emoji namespace
/// sink, unless the typed text is empty (menu just opened) or continues
/// an emoji name, meaning the user is explicitly asking for one.
pub fn demote_julia_emoji_label(filter_text: &str, typed: &str) -> bool {
    match filter_text.strip_prefix("\\:") {
        None => false,
        Some(name) => typed.is_empty() || !name.starts_with(typed),
    }
}

/// Reports whether a completion label lives in Julia's `\:` emoji
/// namespace, which carries no code meaning.
pub fn is_julia_emoji_label(filter_text: &str) -> bool {
    filter_text.starts_with("\\:")
}

#[cfg(test)]
mod tests {
    use super::{demote_julia_emoji_label, is_julia_emoji_label};

    #[test]
    fn emoji_namespace_detected_by_backslash_colon_prefix() {
        assert!(is_julia_emoji_label("\\:apple:"));
        assert!(is_julia_emoji_label("\\:"));
    }

    #[test]
    fn latex_code_and_symbol_labels_are_not_emoji() {
        for label in ["\\alpha", "\\^a", "println", ":symbol", ""] {
            assert!(!is_julia_emoji_label(label), "{label} must not demote");
        }
    }

    #[test]
    fn fuzzy_matched_emoji_sink_but_explicit_requests_keep_place() {
        assert!(demote_julia_emoji_label("\\:apple:", ""));
        assert!(demote_julia_emoji_label("\\:apple:", "alp"));
        assert!(!demote_julia_emoji_label("\\:apple:", "apple"));
        assert!(!demote_julia_emoji_label("\\:apple:", "app"));
    }

    #[test]
    fn real_symbols_never_sink() {
        for typed in ["", "a", "alp", "alpha"] {
            assert!(!demote_julia_emoji_label("\\alpha", typed));
            assert!(!demote_julia_emoji_label("println", typed));
        }
    }
}
