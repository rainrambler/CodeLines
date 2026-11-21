/// Analyzer module: parses file content line by line, classifying each line
/// as code, comment, blank, or mixed (code + comment).
use std::collections::HashMap;

use crate::language::{
    self, BlockComment, BoundaryAfter, CharLiteral, CommentRule, EmbeddedKind, Heredoc,
    LineComment, LineScope, RawString, RegionComment, StringEscape, StringLiteral,
};

/// Per-file line-level statistics result
#[derive(Debug, Clone, Default)]
pub struct LineStats {
    /// Total lines in the file
    pub total_lines: usize,
    /// Pure code lines (without comments)
    pub code_lines: usize,
    /// Pure comment lines (without code)
    pub comment_lines: usize,
    /// Mixed lines (code + comment on the same line)
    pub mixed_lines: usize,
    /// Blank lines
    pub blank_lines: usize,
}

impl std::ops::Add for LineStats {
    type Output = LineStats;

    fn add(self, rhs: LineStats) -> LineStats {
        LineStats {
            total_lines: self.total_lines + rhs.total_lines,
            code_lines: self.code_lines + rhs.code_lines,
            comment_lines: self.comment_lines + rhs.comment_lines,
            mixed_lines: self.mixed_lines + rhs.mixed_lines,
            blank_lines: self.blank_lines + rhs.blank_lines,
        }
    }
}

impl std::ops::AddAssign for LineStats {
    fn add_assign(&mut self, rhs: LineStats) {
        self.total_lines += rhs.total_lines;
        self.code_lines += rhs.code_lines;
        self.comment_lines += rhs.comment_lines;
        self.mixed_lines += rhs.mixed_lines;
        self.blank_lines += rhs.blank_lines;
    }
}

/// Classification of a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Code,
    Comment,
    Mixed,
    Blank,
}

impl LineStats {
    /// Record one classified line.
    pub fn record(&mut self, kind: LineKind) {
        self.total_lines += 1;
        match kind {
            LineKind::Code => self.code_lines += 1,
            LineKind::Comment => self.comment_lines += 1,
            LineKind::Mixed => self.mixed_lines += 1,
            LineKind::Blank => self.blank_lines += 1,
        }
    }

    /// Effective code lines (pure code + mixed)
    pub fn effective_code_lines(&self) -> usize {
        self.code_lines + self.mixed_lines
    }

    /// Total comment lines (pure comment + mixed)
    pub fn total_comment_lines(&self) -> usize {
        self.comment_lines + self.mixed_lines
    }

    /// Code ratio
    pub fn code_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.effective_code_lines() as f64 / self.total_lines as f64
        }
    }

    /// Comment ratio
    pub fn comment_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.total_comment_lines() as f64 / self.total_lines as f64
        }
    }
}

/// Compiled form of a [`CommentRule`] used by the analyzer.
#[derive(Debug, Clone)]
pub struct CompiledSyntax {
    strings: Vec<StringLiteral>,
    raw_strings: Vec<RawString>,
    blocks: Vec<BlockComment>,
    lines: Vec<LineComment>,
    chars: Vec<CharLiteral>,
    regions: Vec<RegionComment>,
    heredocs: Vec<Heredoc>,
}

impl CompiledSyntax {
    pub fn new(rule: &CommentRule) -> Self {
        let mut strings = rule.strings.clone();
        strings.extend(rule.string_delimiters.iter().map(|d| StringLiteral::new(d)));
        strings.sort_by_key(|a| std::cmp::Reverse(a.open.chars().count()));

        let mut blocks = rule.blocks.clone();
        if let (Some(start), Some(end)) = (rule.block_comment_start, rule.block_comment_end) {
            blocks.push(BlockComment::new(start, end));
        }
        blocks.sort_by_key(|a| std::cmp::Reverse(a.start.chars().count()));

        let mut lines = rule.line.clone();
        lines.extend(rule.line_comments.iter().map(|p| LineComment::new(p)));
        lines.sort_by_key(|a| std::cmp::Reverse(a.prefix.chars().count()));

        let mut regions = rule.regions.clone();
        regions.sort_by_key(|a| std::cmp::Reverse(a.start.chars().count()));

        let mut heredocs = rule.heredoc_rules.clone();
        heredocs.extend(rule.heredocs.iter().map(|marker| Heredoc::new(marker)));

        Self {
            strings,
            raw_strings: rule.raw_strings.clone(),
            blocks,
            lines,
            chars: rule.chars.clone(),
            regions,
            heredocs,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HeredocStrip {
    None,
    Tabs,
    Whitespace,
}

#[derive(Debug, Clone, Default)]
enum Mode {
    #[default]
    Normal,
    Block {
        rule: usize,
        depth: usize,
    },
    String {
        rule: usize,
    },
    RawString {
        close: String,
    },
    Region {
        rule: usize,
    },
    Heredoc {
        terminator: String,
        strip: HeredocStrip,
    },
}

#[derive(Debug, Clone, Default)]
struct ParseState {
    mode: Mode,
}

/// Analyze a single file's content, returning per-line statistics.
pub fn analyze_content(content: &str, rule: &CommentRule) -> LineStats {
    analyze_content_compiled(content, &CompiledSyntax::new(rule))
}

/// Same as [`analyze_content`], but reuses an already compiled syntax.
pub fn analyze_content_compiled(content: &str, syntax: &CompiledSyntax) -> LineStats {
    let mut stats = LineStats::default();
    let mut state = ParseState::default();

    for raw_line in content.lines() {
        if raw_line.trim().is_empty() {
            stats.record(LineKind::Blank);
            continue;
        }

        let (has_code, has_comment, new_state) = parse_line(raw_line, syntax, &state);
        state = new_state;
        stats.record(classify(has_code, has_comment));
    }

    stats
}

fn classify(has_code: bool, has_comment: bool) -> LineKind {
    match (has_code, has_comment) {
        (true, true) => LineKind::Mixed,
        (true, false) => LineKind::Code,
        (false, true) => LineKind::Comment,
        (false, false) => LineKind::Blank,
    }
}

/// Parse one line of text, returning (has_code, has_comment, new_state)
fn parse_line(line: &str, syntax: &CompiledSyntax, state: &ParseState) -> (bool, bool, ParseState) {
    // Delimiters are ASCII, so scanning bytes directly is safe: UTF-8
    // continuation bytes (>= 0x80) can never form an ASCII marker.
    let bytes = line.as_bytes();
    let len = bytes.len();
    let first_nonspace = bytes.iter().position(|b| !b.is_ascii_whitespace());
    let mut pos = 0;
    let mut has_code = false;
    let mut has_comment = false;
    let mut mode = state.mode.clone();
    let mut pending_heredoc = None;

    if let Mode::Heredoc { terminator, strip } = mode.clone() {
        let candidate = match strip {
            HeredocStrip::None => line,
            HeredocStrip::Tabs => line.trim_start_matches('\t'),
            HeredocStrip::Whitespace => line.trim_start(),
        };
        if candidate.trim_end() == terminator {
            return (true, false, ParseState { mode: Mode::Normal });
        }
        return (
            true,
            false,
            ParseState {
                mode: Mode::Heredoc { terminator, strip },
            },
        );
    }

    loop {
        match std::mem::take(&mut mode) {
            Mode::Block { rule, depth } => {
                let block = &syntax.blocks[rule];
                if pos >= len {
                    mode = Mode::Block { rule, depth };
                    break;
                }
                if scope_matches(block.end_scope, bytes, pos, first_nonspace, block.end.len())
                    && starts_with(bytes, pos, block.end)
                {
                    has_comment = true;
                    pos += block.end.len();
                    mode = if depth <= 1 {
                        Mode::Normal
                    } else {
                        Mode::Block {
                            rule,
                            depth: depth - 1,
                        }
                    };
                    continue;
                }
                if block.nested && starts_with(bytes, pos, block.start) {
                    has_comment = true;
                    pos += block.start.len();
                    mode = Mode::Block {
                        rule,
                        depth: depth + 1,
                    };
                    continue;
                }
                has_comment = true;
                pos += 1;
                mode = Mode::Block { rule, depth };
                continue;
            }
            Mode::String { rule } => {
                let string = &syntax.strings[rule];
                if pos >= len {
                    mode = Mode::String { rule };
                    break;
                }
                let escape = string.escape;
                if escape == StringEscape::Backslash && bytes[pos] == b'\\' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    mode = Mode::String { rule };
                    continue;
                }
                if escape == StringEscape::Backtick && bytes[pos] == b'`' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    mode = Mode::String { rule };
                    continue;
                }
                if scope_matches(
                    string.close_scope,
                    bytes,
                    pos,
                    first_nonspace,
                    string.close.len(),
                ) && starts_with(bytes, pos, string.close)
                {
                    let close_len = string.close.len();
                    if escape == StringEscape::DoubledQuote
                        && starts_with(bytes, pos + close_len, string.close)
                    {
                        has_code = true;
                        pos += close_len * 2;
                        mode = Mode::String { rule };
                        continue;
                    }
                    has_code = true;
                    pos += close_len;
                    mode = Mode::Normal;
                    continue;
                }
                has_code = true;
                pos += 1;
                mode = Mode::String { rule };
                continue;
            }
            Mode::RawString { close } => {
                if pos >= len {
                    mode = Mode::RawString { close };
                    break;
                }
                if starts_with(bytes, pos, &close) {
                    has_code = true;
                    pos += close.len();
                    mode = Mode::Normal;
                    continue;
                }
                has_code = true;
                pos += 1;
                mode = Mode::RawString { close };
                continue;
            }
            Mode::Region { rule } => {
                let region = &syntax.regions[rule];
                if pos >= len {
                    mode = Mode::Region { rule };
                    break;
                }
                if scope_matches(
                    region.end_scope,
                    bytes,
                    pos,
                    first_nonspace,
                    region.end.len(),
                ) && starts_with(bytes, pos, region.end)
                {
                    has_comment = true;
                    pos += region.end.len();
                    mode = Mode::Normal;
                    continue;
                }
                has_comment = true;
                pos += 1;
                mode = Mode::Region { rule };
                continue;
            }
            Mode::Heredoc { terminator, strip } => {
                mode = Mode::Heredoc { terminator, strip };
                break;
            }
            Mode::Normal => {}
        }

        if pos >= len {
            break;
        }

        // Whitespace never counts as code, otherwise indentation before a
        // comment would turn a pure comment line into a mixed line.
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        if bytes[pos] == b'\\' && pos + 1 < len {
            has_code = true;
            pos += 2;
            continue;
        }

        if let Some((next, terminator, strip)) = match_heredoc(bytes, pos, syntax) {
            has_code = true;
            pos = next;
            pending_heredoc = Some(Mode::Heredoc { terminator, strip });
            continue;
        }

        let mut matched = false;
        for raw in &syntax.raw_strings {
            if !starts_with(bytes, pos, raw.prefix) {
                continue;
            }
            if let Some((next, close)) = match_raw_string(bytes, pos, raw) {
                has_code = true;
                pos = next;
                mode = Mode::RawString { close };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        if let Some(next) = match_char_literal(bytes, pos, &syntax.chars) {
            has_code = true;
            pos = next;
            continue;
        }

        for (index, string) in syntax.strings.iter().enumerate() {
            if starts_with(bytes, pos, string.open) && string_open_allowed(string, bytes, pos) {
                has_code = true;
                pos += string.open.len();
                mode = Mode::String { rule: index };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for (index, block) in syntax.blocks.iter().enumerate() {
            if scope_matches(
                block.start_scope,
                bytes,
                pos,
                first_nonspace,
                block.start.len(),
            ) && starts_with(bytes, pos, block.start)
            {
                has_comment = true;
                pos += block.start.len();
                mode = Mode::Block {
                    rule: index,
                    depth: 1,
                };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for (index, region) in syntax.regions.iter().enumerate() {
            if scope_matches(
                region.start_scope,
                bytes,
                pos,
                first_nonspace,
                region.start.len(),
            ) && starts_with(bytes, pos, region.start)
            {
                has_comment = true;
                pos += region.start.len();
                mode = Mode::Region { rule: index };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for line_comment in &syntax.lines {
            if line_comment_matches(bytes, pos, first_nonspace, line_comment) {
                has_comment = true;
                pos = len;
                matched = true;
                break;
            }
        }
        if matched {
            break;
        }

        has_code = true;
        pos += 1;
    }

    let reset_single_line_string = match &mode {
        Mode::String { rule } => !syntax.strings[*rule].multiline,
        _ => false,
    };
    if reset_single_line_string {
        mode = Mode::Normal;
    }

    let mode = pending_heredoc.unwrap_or(mode);
    (has_code, has_comment, ParseState { mode })
}

fn scope_matches(
    scope: LineScope,
    bytes: &[u8],
    pos: usize,
    first_nonspace: Option<usize>,
    prefix_len: usize,
) -> bool {
    match scope {
        LineScope::Anywhere => true,
        LineScope::StartOfLine => pos == 0,
        LineScope::FirstNonSpace => Some(pos) == first_nonspace,
        LineScope::AloneOnLine => {
            Some(pos) == first_nonspace
                && bytes
                    .get(pos + prefix_len..)
                    .is_none_or(|rest| rest.iter().all(|b| b.is_ascii_whitespace()))
        }
        LineScope::Column(column) => pos == column,
    }
}

fn string_open_allowed(string: &StringLiteral, bytes: &[u8], pos: usize) -> bool {
    if !string.open_requires_line_end {
        return true;
    }
    bytes
        .get(pos + string.open.len()..)
        .is_none_or(|rest| rest.iter().all(|b| b.is_ascii_whitespace()))
}

fn line_comment_matches(
    bytes: &[u8],
    pos: usize,
    first_nonspace: Option<usize>,
    rule: &LineComment,
) -> bool {
    if !scope_matches(rule.scope, bytes, pos, first_nonspace, rule.prefix.len()) {
        return false;
    }
    if rule.require_whitespace_before && pos > 0 {
        let previous = bytes[pos - 1];
        if !previous.is_ascii_whitespace() && !matches!(previous, b';' | b'|' | b'&' | b'(' | b')')
        {
            return false;
        }
    }
    let matched = if rule.case_insensitive {
        starts_with_ignore_case(bytes, pos, rule.prefix)
    } else {
        starts_with(bytes, pos, rule.prefix)
    };
    if !matched {
        return false;
    }
    match rule.boundary_after {
        None => true,
        Some(BoundaryAfter::Word) => bytes
            .get(pos + rule.prefix.len())
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_')),
        Some(BoundaryAfter::Symbol) => bytes
            .get(pos + rule.prefix.len())
            .is_none_or(|b| !is_haskell_symbol(*b)),
    }
}

fn match_heredoc(
    bytes: &[u8],
    pos: usize,
    syntax: &CompiledSyntax,
) -> Option<(usize, String, HeredocStrip)> {
    for rule in &syntax.heredocs {
        if !starts_with(bytes, pos, rule.marker) {
            continue;
        }
        // `<<<` is a here-string in shells, not a heredoc.
        if rule.marker == "<<" && bytes.get(pos + 2) == Some(&b'<') {
            continue;
        }
        let mut index = pos + rule.marker.len();
        let mut strip = HeredocStrip::None;
        match bytes.get(index) {
            Some(b'-') => {
                strip = HeredocStrip::Tabs;
                index += 1;
            }
            Some(b'~') => {
                strip = HeredocStrip::Whitespace;
                index += 1;
            }
            _ => {}
        }
        if bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
            if !rule.allow_space {
                continue;
            }
            while bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
                index += 1;
            }
        }
        let quote = match bytes.get(index) {
            Some(b'\'') => Some(b'\''),
            Some(b'"') => Some(b'"'),
            _ => None,
        };
        if quote.is_some() {
            index += 1;
        }
        let start = index;
        if !bytes
            .get(start)
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        {
            continue;
        }
        if quote.is_none() && rule.require_uppercase && !bytes[start].is_ascii_uppercase() {
            continue;
        }
        while index < bytes.len() {
            let current = bytes[index];
            if current.is_ascii_whitespace()
                || matches!(current, b';' | b'|' | b'&' | b'(' | b')')
                || Some(current) == quote
            {
                break;
            }
            index += 1;
        }
        if index == start {
            continue;
        }
        let terminator = String::from_utf8_lossy(&bytes[start..index]).into_owned();
        if let Some(quote) = quote {
            if bytes.get(index) != Some(&quote) {
                continue;
            }
            index += 1;
        }
        return Some((index, terminator, strip));
    }
    None
}

fn match_raw_string(bytes: &[u8], pos: usize, rule: &RawString) -> Option<(usize, String)> {
    if !rule.open.is_ascii() || !rule.close.is_ascii() {
        return None;
    }
    let open = rule.open as u8;
    let mut index = pos + rule.prefix.len();
    let delimiter_start = index;
    while index < bytes.len() {
        let current = bytes[index];
        if current == open {
            break;
        }
        if index - delimiter_start >= 16
            || current.is_ascii_whitespace()
            || matches!(current, b'\\' | b')' | b'"' | b'{' | b'}')
        {
            return None;
        }
        index += 1;
    }
    if bytes.get(index) != Some(&open) {
        return None;
    }
    let delimiter = std::str::from_utf8(&bytes[delimiter_start..index]).ok()?;
    Some((index + 1, format!("{}{}\"", rule.close, delimiter)))
}

fn match_char_literal(bytes: &[u8], pos: usize, rules: &[CharLiteral]) -> Option<usize> {
    for rule in rules {
        if !rule.quote.is_ascii() || bytes.get(pos) != Some(&(rule.quote as u8)) {
            continue;
        }
        if let Some(end) = char_literal_end(bytes, pos, *rule) {
            return Some(end);
        }
    }
    None
}

fn char_literal_end(bytes: &[u8], pos: usize, rule: CharLiteral) -> Option<usize> {
    let quote = rule.quote as u8;
    let index = pos + 1;
    if index >= bytes.len() {
        return None;
    }
    if bytes[index] == b'\\' {
        let limit = (index + 24).min(bytes.len());
        return bytes[index + 1..limit]
            .iter()
            .position(|current| *current == quote)
            .map(|offset| index + 1 + offset + 1);
    }
    let limit = (pos + 2 + rule.max_len).min(bytes.len());
    for (offset, current) in bytes[pos + 1..limit].iter().enumerate() {
        if *current == quote {
            return Some(pos + 1 + offset + 1);
        }
        if current.is_ascii_whitespace() {
            return None;
        }
    }
    None
}

fn is_haskell_symbol(current: u8) -> bool {
    matches!(
        current,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'*'
            | b'+'
            | b'.'
            | b'/'
            | b'<'
            | b'='
            | b'>'
            | b'?'
            | b'@'
            | b'\\'
            | b'^'
            | b'|'
            | b'-'
            | b'~'
            | b':'
    )
}

/// Returns true if bytes[pos..] starts with target
fn starts_with(bytes: &[u8], pos: usize, target: &str) -> bool {
    target
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(offset, expected)| bytes.get(pos + offset) == Some(expected))
}

fn starts_with_ignore_case(bytes: &[u8], pos: usize, target: &str) -> bool {
    target
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(offset, expected)| {
            bytes
                .get(pos + offset)
                .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
        })
}

/// A contiguous block of embedded code inside a host file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    pub kind: String,
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Analyze a file whose host language contains embedded source regions, such as
/// `<script>`/`<style>` blocks in HTML or fenced code blocks in Markdown.
pub fn analyze_embedded(
    content: &str,
    host_syntax: &CompiledSyntax,
    kind: EmbeddedKind,
) -> (LineStats, Vec<RegionInfo>) {
    match kind {
        EmbeddedKind::Html => analyze_html(content, host_syntax),
        EmbeddedKind::Markdown => analyze_markdown(content, host_syntax),
    }
}

struct SyntaxCache {
    syntaxes: HashMap<&'static str, CompiledSyntax>,
    states: HashMap<&'static str, ParseState>,
    host_state: ParseState,
}

impl SyntaxCache {
    fn new() -> Self {
        Self {
            syntaxes: HashMap::new(),
            states: HashMap::new(),
            host_state: ParseState::default(),
        }
    }

    fn reset_embedded(&mut self, language_name: &'static str) {
        self.states.remove(language_name);
    }

    fn host_can_start_script(&self) -> bool {
        // Only block/comment state suppresses tag detection. String state in
        // markup is often a false positive (for example an apostrophe in text)
        // and must not hide a later <script> block.
        !matches!(self.host_state.mode, Mode::Block { .. })
    }

    fn analyze_host(&mut self, line: &str, host_syntax: &CompiledSyntax) -> LineKind {
        if line.trim().is_empty() {
            return LineKind::Blank;
        }
        let (has_code, has_comment, new_state) = parse_line(line, host_syntax, &self.host_state);
        self.host_state = new_state;
        classify(has_code, has_comment)
    }

    fn analyze_embedded(&mut self, line: &str, language_name: &'static str) -> LineKind {
        if line.trim().is_empty() {
            return LineKind::Blank;
        }
        let syntax = self.syntaxes.entry(language_name).or_insert_with(|| {
            language::find_language_by_name(language_name)
                .map(|definition| CompiledSyntax::new(&definition.comment_rule))
                .unwrap_or_else(|| CompiledSyntax::new(&CommentRule::default()))
        });
        let state = self.states.entry(language_name).or_default();
        let (has_code, has_comment, new_state) = parse_line(line, syntax, state);
        *state = new_state;
        classify(has_code, has_comment)
    }
}

struct OpenRegion {
    tag: &'static str,
    kind: &'static str,
    language: &'static str,
    start_line: usize,
}

impl OpenRegion {
    fn into_info(self, end_line: usize) -> RegionInfo {
        RegionInfo {
            kind: self.kind.to_string(),
            language: self.language.to_string(),
            start_line: self.start_line,
            end_line,
        }
    }
}

struct OpeningTag {
    tag: &'static str,
    kind: &'static str,
    language: &'static str,
    has_content: bool,
    has_close: bool,
    self_closing: bool,
}

fn analyze_html(content: &str, host_syntax: &CompiledSyntax) -> (LineStats, Vec<RegionInfo>) {
    let mut cache = SyntaxCache::new();
    let mut stats = LineStats::default();
    let mut regions = Vec::new();
    let mut open: Option<OpenRegion> = None;
    let mut last_line = 0;

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        last_line = line_number;

        if let Some(current) = open.as_ref() {
            stats.record(cache.analyze_embedded(line, current.language));
            if contains_closing_tag(line, current.tag) {
                regions.push(RegionInfo {
                    kind: current.kind.to_string(),
                    language: current.language.to_string(),
                    start_line: current.start_line,
                    end_line: line_number,
                });
                cache.reset_embedded(current.language);
                open = None;
            }
            continue;
        }

        if let Some(tag) = cache
            .host_can_start_script()
            .then(|| find_opening_tag(line))
            .flatten()
        {
            cache.reset_embedded(tag.language);
            let line_kind = if tag.has_content {
                cache.analyze_embedded(line, tag.language)
            } else {
                cache.analyze_host(line, host_syntax)
            };
            stats.record(line_kind);

            if tag.has_close {
                if tag.has_content && !tag.self_closing {
                    regions.push(RegionInfo {
                        kind: tag.kind.to_string(),
                        language: tag.language.to_string(),
                        start_line: line_number,
                        end_line: line_number,
                    });
                }
            } else if !tag.self_closing {
                open = Some(OpenRegion {
                    tag: tag.tag,
                    kind: tag.kind,
                    language: tag.language,
                    start_line: line_number,
                });
            }
            continue;
        }

        stats.record(cache.analyze_host(line, host_syntax));
    }

    if let Some(current) = open {
        let end_line = last_line.max(current.start_line);
        regions.push(current.into_info(end_line));
    }

    (stats, regions)
}

fn analyze_markdown(content: &str, host_syntax: &CompiledSyntax) -> (LineStats, Vec<RegionInfo>) {
    let mut cache = SyntaxCache::new();
    let mut stats = LineStats::default();
    let mut regions = Vec::new();
    let mut open: Option<FenceRegion> = None;
    let mut last_line = 0;

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        last_line = line_number;

        if let Some(fence) = open.as_ref() {
            if is_fence_close(line, fence.marker, fence.length) {
                stats.record(LineKind::Code);
                regions.push(RegionInfo {
                    kind: "fence".to_string(),
                    language: fence.language.to_string(),
                    start_line: fence.start_line,
                    end_line: line_number,
                });
                cache.reset_embedded(fence.language);
                open = None;
            } else {
                stats.record(cache.analyze_embedded(line, fence.language));
            }
            continue;
        }

        if let Some(fence) = fence_open(line) {
            cache.reset_embedded(fence.language);
            stats.record(LineKind::Code);
            open = Some(FenceRegion {
                marker: fence.marker,
                length: fence.length,
                language: fence.language,
                start_line: line_number,
            });
            continue;
        }

        stats.record(cache.analyze_host(line, host_syntax));
    }

    if let Some(fence) = open {
        regions.push(RegionInfo {
            kind: "fence".to_string(),
            language: fence.language.to_string(),
            start_line: fence.start_line,
            end_line: last_line.max(fence.start_line),
        });
    }

    (stats, regions)
}

struct FenceRegion {
    marker: char,
    length: usize,
    language: &'static str,
    start_line: usize,
}

struct FenceOpen {
    marker: char,
    length: usize,
    language: &'static str,
}

fn fence_open(line: &str) -> Option<FenceOpen> {
    let (marker, run) = fence_marker(line)?;
    let length = run.len();
    if length < 3 {
        return None;
    }
    let info = line[run.end..].trim();
    let language = language::language_for_info_string(info)
        .map(|definition| definition.name)
        .unwrap_or("Text");
    Some(FenceOpen {
        marker,
        length,
        language,
    })
}

fn fence_marker(line: &str) -> Option<(char, std::ops::Range<usize>)> {
    let leading = line.len() - line.trim_start_matches(' ').len();
    if leading > 3 {
        return None;
    }
    let rest = &line[leading..];
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = rest.chars().take_while(|c| *c == marker).count();
    let end = leading + length;
    if marker == '`' && rest[length..].contains('`') {
        return None;
    }
    Some((marker, leading..end))
}

fn is_fence_close(line: &str, marker: char, minimum: usize) -> bool {
    let leading = line.len() - line.trim_start_matches(' ').len();
    if leading > 3 {
        return false;
    }
    let rest = &line[leading..];
    let run = rest.chars().take_while(|c| *c == marker).count();
    run >= minimum && rest[run..].trim().is_empty()
}

fn find_opening_tag(line: &str) -> Option<OpeningTag> {
    let lower = line.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for tag in ["script", "style"] {
        if let Some(index) = find_tag_start(&lower, tag) {
            candidates.push((index, tag));
        }
    }
    candidates.sort_by_key(|(index, _)| *index);
    candidates
        .into_iter()
        .find_map(|(index, tag)| parse_opening_tag(line, &lower, index, tag))
}

fn find_tag_start(lower: &str, tag: &str) -> Option<usize> {
    let needle = format!("<{tag}");
    let mut from = 0;
    while let Some(offset) = lower[from..].find(&needle) {
        let index = from + offset;
        let boundary = lower.as_bytes().get(index + needle.len()).copied();
        if boundary.is_none_or(|byte| byte.is_ascii_whitespace() || byte == b'>' || byte == b'/') {
            return Some(index);
        }
        from = index + needle.len();
    }
    None
}

fn parse_opening_tag(
    line: &str,
    lower: &str,
    index: usize,
    tag: &'static str,
) -> Option<OpeningTag> {
    let relative_end = lower[index..].find('>')?;
    let end = index + relative_end;
    let tag_text = &lower[index..=end];
    let language = tag_language(tag, tag_text)?;
    let self_closing = tag_text.trim_end().ends_with("/>");
    let after = &line[end + 1..];
    let close_marker = format!("</{tag}");
    let close_offset = after.to_ascii_lowercase().find(&close_marker);
    let inner = match close_offset {
        Some(offset) => &after[..offset],
        None => after,
    };
    Some(OpeningTag {
        tag,
        kind: tag,
        language,
        has_content: !inner.trim().is_empty(),
        has_close: close_offset.is_some(),
        self_closing,
    })
}

fn tag_language(tag: &str, tag_text: &str) -> Option<&'static str> {
    let lang = attribute_value(tag_text, "lang");
    if tag == "style" {
        if attribute_value(tag_text, "type").as_deref() == Some("text/stylus") {
            return Some("Stylus");
        }
        return Some(match lang.as_deref() {
            Some("stylus") => "Stylus",
            _ => "CSS",
        });
    }

    if let Some(lang) = lang {
        return match lang.as_str() {
            "ts" | "typescript" | "tsx" | "js" | "javascript" | "jsx" => {
                Some("JavaScript/TypeScript")
            }
            _ => None,
        };
    }

    if let Some(kind) = attribute_value(tag_text, "type") {
        let kind = kind.split(';').next().unwrap_or_default().trim();
        return match kind {
            "module"
            | "text/javascript"
            | "application/javascript"
            | "text/ecmascript"
            | "application/ecmascript"
            | "text/typescript"
            | "application/typescript" => Some("JavaScript/TypeScript"),
            _ => None,
        };
    }

    Some("JavaScript/TypeScript")
}

fn attribute_value(tag_text: &str, name: &str) -> Option<String> {
    let bytes = tag_text.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len()
        && !bytes[cursor].is_ascii_whitespace()
        && bytes[cursor] != b'>'
        && bytes[cursor] != b'/'
    {
        cursor += 1;
    }

    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'/')
        {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] == b'>' {
            break;
        }

        let name_start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && bytes[cursor] != b'='
            && bytes[cursor] != b'>'
            && bytes[cursor] != b'/'
        {
            cursor += 1;
        }
        let attribute_name = &tag_text[name_start..cursor];

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let (value_start, value_end) = match bytes.get(cursor).copied() {
            Some(quote @ (b'"' | b'\'')) => {
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                let end = cursor;
                if cursor < bytes.len() {
                    cursor += 1;
                }
                (start, end)
            }
            Some(_) => {
                let start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'>'
                {
                    cursor += 1;
                }
                (start, cursor)
            }
            None => break,
        };

        if attribute_name.eq_ignore_ascii_case(name) {
            return Some(tag_text[value_start..value_end].trim().to_ascii_lowercase());
        }
    }

    None
}

fn contains_closing_tag(line: &str, tag: &str) -> bool {
    let bytes = line.as_bytes();
    let tag = tag.as_bytes();
    bytes.windows(2 + tag.len()).any(|window| {
        window[0] == b'<' && window[1] == b'/' && window[2..].eq_ignore_ascii_case(tag)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_rule() -> CommentRule {
        CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("/*", "*/").nested())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::raw("r#\"", "\"#"))
            .char_rule('\'', 1)
    }

    fn python_rule() -> CommentRule {
        CommentRule {
            line_comments: vec!["#"],
            block_comment_start: Some("\"\"\""),
            block_comment_end: Some("\"\"\""),
            string_delimiters: vec!["\"\"\"", "'''", "\"", "'"],
            ..Default::default()
        }
    }

    fn shell_rule() -> CommentRule {
        CommentRule::default()
            .line_rule(LineComment::new("#").require_whitespace_before())
            .string_rule(StringLiteral::with_escape(
                "\"",
                "\"",
                StringEscape::Backslash,
            ))
            .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
            .heredoc("<<")
    }

    fn html_rule() -> CommentRule {
        CommentRule {
            line_comments: vec![],
            block_comment_start: Some("<!--"),
            block_comment_end: Some("-->"),
            string_delimiters: vec!["\"", "'"],
            ..Default::default()
        }
    }

    fn in_block(state: &ParseState) -> bool {
        matches!(state.mode, Mode::Block { .. })
    }

    fn in_string(state: &ParseState) -> bool {
        matches!(state.mode, Mode::String { .. })
    }

    fn default_state() -> ParseState {
        ParseState::default()
    }

    fn parse(line: &str, rule: &CommentRule, state: &ParseState) -> (bool, bool, ParseState) {
        parse_line(line, &CompiledSyntax::new(rule), state)
    }

    #[test]
    fn test_rust_line_comment() {
        let (code, comment, state) = parse("// this is a comment", &rust_rule(), &default_state());
        assert!(!code);
        assert!(comment);
        assert!(!in_block(&state));
        assert!(!in_string(&state));
    }

    #[test]
    fn test_rust_code_line() {
        let (code, comment, state) = parse("let x = 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(!comment);
        assert!(!in_block(&state));
    }

    #[test]
    fn test_rust_mixed_line() {
        let (code, comment, _) = parse("let x = 5; // comment", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
    }

    #[test]
    fn test_string_with_comment_chars() {
        let (code, comment, _) = parse(
            "let s = \"// not a comment\";",
            &rust_rule(),
            &default_state(),
        );
        assert!(code);
        assert!(!comment);
    }

    #[test]
    fn test_python_triple_quote_string() {
        let (code, _comment, state) = parse("\"\"\"docstring", &python_rule(), &default_state());
        assert!(in_string(&state));
        assert!(code);
    }

    #[test]
    fn test_python_multiline_string_continuation() {
        let (_, _, state) = parse("\"\"\"", &python_rule(), &default_state());
        assert!(in_string(&state));

        let (code, comment, state) = parse("this is inside the string", &python_rule(), &state);
        assert!(code);
        assert!(!comment);
        assert!(in_string(&state));

        let (code, _, state) = parse("\"\"\"", &python_rule(), &state);
        assert!(code);
        assert!(!in_string(&state));
    }

    #[test]
    fn test_full_rust_file() {
        let content = r#"// A simple Rust program
fn main() {
    /* multi-line
       comment */
    let x = 5; // inline
}
"#;
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.total_lines, 6);
        assert_eq!(stats.blank_lines, 0);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 2);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_python_file() {
        let content = r#""""This is a docstring"""
def hello():
    # a comment
    print("hello")
"#;
        let stats = analyze_content(content, &python_rule());
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_shell_comment() {
        let (code, comment, _) = parse("# this is a comment", &shell_rule(), &default_state());
        assert!(!code);
        assert!(comment);
    }

    #[test]
    fn test_html_comment() {
        let (code, comment, _) = parse("<!-- comment -->", &html_rule(), &default_state());
        assert!(!code);
        assert!(comment);
    }

    #[test]
    fn test_rust_block_comment_multiline() {
        let content = "/* this is\na block\ncomment */\ncode_here\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_rust_code_with_block_comment_on_same_line() {
        let (code, comment, state) =
            parse("let x = /* inline */ 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
        assert!(!in_block(&state));
    }

    #[test]
    fn test_rust_lifetime_is_not_a_string() {
        let content = "let x: &'a str = \"value\"; // real comment\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.mixed_lines, 1);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_rust_raw_string_ignores_comment_markers() {
        let content = "let s = r#\"// not a comment\"#;\n// real comment\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.mixed_lines, 0);
    }

    #[test]
    fn test_nested_block_comments() {
        let content = "/* outer /* inner */ still outer */\nfn main() {}\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.mixed_lines, 0);
    }

    #[test]
    fn test_vim_double_quote_is_a_comment() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("\""))
            .string_rule(StringLiteral::doubled("'", "'").single_line());
        let content = "\" a comment\nlet x = 'a''b'\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_smalltalk_string_is_a_comment() {
        let rule = CommentRule::default()
            .block_rule(BlockComment::new("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "\"a comment\"\nTranscript show: 'it''s'\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_shell_hash_requires_word_boundary() {
        let content = "echo ${#var}\necho hi # real comment\n";
        let stats = analyze_content(content, &shell_rule());
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_shell_heredoc_body_is_code() {
        let content = "cat <<EOF\n# not a comment\nEOF\n";
        let stats = analyze_content(content, &shell_rule());
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_haskell_operator_is_not_a_comment() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
            .block_rule(BlockComment::new("{-", "-}").nested())
            .string_rule(StringLiteral::new("\""))
            .char_rule('\'', 1);
        let content = "a --> b\n-- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_batch_rem_is_case_insensitive() {
        let rule = CommentRule::default()
            .line_rule(
                LineComment::first_non_space("rem")
                    .case_insensitive()
                    .with_boundary_after(BoundaryAfter::Word),
            )
            .line_rule(LineComment::first_non_space("::"))
            .string_rule(StringLiteral::new("\""));
        let content = "REM a comment\nremove file.txt\n:: label comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_sql_string_with_comment_marker() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--"))
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "SELECT '-- not a comment' -- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_char_literals_and_escapes() {
        let content = "let c = '\\n'; // real comment\nlet f = '\\u{1F600}';\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.mixed_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_multiple_block_syntaxes() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("{", "}"))
            .block_rule(BlockComment::new("(*", "*)"))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "{ comment }\n(* another *)\nwriteln('x');\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_lua_long_comments_and_raw_strings() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--"))
            .block_rule(BlockComment::new("--[[", "]]"))
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .string_rule(StringLiteral::raw("[[", "]]"));
        let content = "--[[ block\nstill ]]\nlocal s = [[-- not a comment]]\n-- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_powershell_here_string() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .block_rule(BlockComment::new("<#", "#>"))
            .string_rule(StringLiteral::with_escape(
                "\"",
                "\"",
                StringEscape::Backtick,
            ))
            .string_rule(StringLiteral::doubled("'", "'"))
            .string_rule(
                StringLiteral::with_escape("@\"", "\"@", StringEscape::Backtick)
                    .close_at_start()
                    .open_at_line_end(),
            )
            .string_rule(
                StringLiteral::with_escape("@'", "'@", StringEscape::None)
                    .close_at_start()
                    .open_at_line_end(),
            );
        let content = "$x = @\"\n# not a comment\n\"@\n# real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_perl_pod_region() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .region_rule(RegionComment::new("=", "=cut").at_start().end_at_start())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .heredoc("<<");
        let content = "=pod\n# pod text\n=cut\nprint \"hi\"; # real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_cpp_raw_string_with_custom_delimiter() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::new("\""))
            .raw_string_rule(RawString::cpp())
            .char_rule('\'', 8);

        let single = analyze_content(
            "auto s = R\"delim(// not a comment \" still)delim\"; // real\n",
            &rule,
        );
        assert_eq!(single.mixed_lines, 1);
        assert_eq!(single.comment_lines, 0);

        let multi = analyze_content("auto s = R\"x(\n// inside\n)x\";\n// real\n", &rule);
        assert_eq!(multi.code_lines, 3);
        assert_eq!(multi.comment_lines, 1);
    }

    #[test]
    fn test_ruby_heredoc_and_shift_operator() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .block_rule(
                BlockComment::new("=begin", "=end")
                    .at_start()
                    .end_at_start(),
            )
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .string_rule(StringLiteral::new("`"))
            .heredoc_rule(Heredoc::new("<<").require_uppercase());

        let content = "arr << x\nsql = <<~SQL\n# body\nSQL\n# real\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 4);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_perl_heredoc_and_shift_operator() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .heredoc_rule(Heredoc::new("<<").require_uppercase());

        let content = "$x << 1;\nmy $s = <<EOF;\n# body\nEOF\n# real\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 4);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_shell_heredoc_with_space_after_marker() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#").require_whitespace_before())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
            .heredoc_rule(Heredoc::new("<<").allow_space());

        let stats = analyze_content("cat << EOF\n# body\nEOF\n", &rule);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_cobol_indicator_area() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("*").with_scope(LineScope::Column(6)))
            .line_rule(LineComment::new("/").with_scope(LineScope::Column(6)))
            .line_rule(LineComment::new("*>"))
            .string_rule(StringLiteral::doubled("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));

        let content =
            "      * fixed comment\n      / page comment\n           DISPLAY 'HI'. *> inline\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_sas_statement_comment_region() {
        let rule = CommentRule::default()
            .region_rule(RegionComment::new("*", ";").first_non_space())
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::doubled("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));

        let content = "* statement comment\ncontinues here;\ndata x;\n/* block */\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_unicode_content_does_not_panic() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .string_rule(StringLiteral::new("\""));
        let stats = analyze_content("// 中文注释\nlet s = \"中文 // 不是注释\";\n", &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_html_script_and_style_regions() {
        let content = "<html>\n<script>\n// comment\nconst x = 1;\n</script>\n<style>\n/* css */\nbody { color: red; }\n</style>\n</html>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);

        assert_eq!(
            regions,
            vec![
                RegionInfo {
                    kind: "script".into(),
                    language: "JavaScript/TypeScript".into(),
                    start_line: 2,
                    end_line: 5,
                },
                RegionInfo {
                    kind: "style".into(),
                    language: "CSS".into(),
                    start_line: 6,
                    end_line: 9,
                },
            ]
        );
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.code_lines, 8);
    }

    #[test]
    fn test_vue_script_with_lang_attribute() {
        let content = "<template>\n  <div>{{ x }}</div>\n</template>\n<script lang=\"ts\">\n// ts comment\nconst x: number = 1;\n</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].language, "JavaScript/TypeScript");
        assert_eq!((regions[0].start_line, regions[0].end_line), (4, 7));
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_html_inline_script_region() {
        let content = "<script>const x = 1;</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);

        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].start_line, regions[0].end_line), (1, 1));
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_html_script_inside_comment_is_not_a_region() {
        let content =
            "<!--\n<script>\n// commented out\n</script>\n-->\n<script>\n// real\n</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);

        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].start_line, regions[0].end_line), (6, 8));
        assert_eq!(stats.comment_lines, 6);
    }

    #[test]
    fn test_html_apostrophe_does_not_hide_later_script() {
        let content = "<p>It's fine</p>\n<script>\n// comment\n</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (_, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);

        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].start_line, regions[0].end_line), (2, 4));
    }

    #[test]
    fn test_html_non_javascript_script_is_not_embedded() {
        let content = "<script type=\"application/json\">\n{\"a\": 1}\n</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (_, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);
        assert!(regions.is_empty());
    }

    #[test]
    fn test_html_attributes_do_not_match_inside_values() {
        let content = "<script data-note=\"lang=ts\" type=\"application/json\">\n{}\n</script>\n";
        let host = CompiledSyntax::new(&html_rule());
        let (_, regions) = analyze_embedded(content, &host, EmbeddedKind::Html);
        assert!(regions.is_empty());
    }

    #[test]
    fn test_markdown_fenced_code_blocks() {
        let content = "# Title\n\n```rust\n// comment\nlet x = 1;\n```\n\n~~~python\n# python comment\nprint(1)\n~~~\n";
        let host = CompiledSyntax::new(&CommentRule::default());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Markdown);

        assert_eq!(
            regions,
            vec![
                RegionInfo {
                    kind: "fence".into(),
                    language: "Rust".into(),
                    start_line: 3,
                    end_line: 6,
                },
                RegionInfo {
                    kind: "fence".into(),
                    language: "Python".into(),
                    start_line: 8,
                    end_line: 11,
                },
            ]
        );
        assert_eq!(stats.code_lines, 7);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.blank_lines, 2);
    }

    #[test]
    fn test_markdown_unknown_fence_language() {
        let content = "```\ncode\n```\n";
        let host = CompiledSyntax::new(&CommentRule::default());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Markdown);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].language, "Text");
        assert_eq!(stats.code_lines, 3);
    }

    #[test]
    fn test_markdown_fence_not_closed_by_other_marker() {
        let content = "```rust\nlet x = 1;\n~~~\n```\n";
        let host = CompiledSyntax::new(&CommentRule::default());
        let (_, regions) = analyze_embedded(content, &host, EmbeddedKind::Markdown);

        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].start_line, regions[0].end_line), (1, 4));
    }

    #[test]
    fn test_embedded_state_resets_between_fences() {
        let content = "```rust\n/* unterminated\n```\n\n```rust\nlet x = 1;\n```\n";
        let host = CompiledSyntax::new(&CommentRule::default());
        let (stats, regions) = analyze_embedded(content, &host, EmbeddedKind::Markdown);

        assert_eq!(regions.len(), 2);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 5);
    }

    #[test]
    fn test_markdown_unclosed_fence_runs_to_eof() {
        let content = "```rust\nlet x = 1;\n";
        let host = CompiledSyntax::new(&CommentRule::default());
        let (_, regions) = analyze_embedded(content, &host, EmbeddedKind::Markdown);

        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].start_line, regions[0].end_line), (1, 2));
    }
}
