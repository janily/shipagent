//! Bracket mentions ported from mkagent v0.2.6 renderer/lib/mentions.ts.
//! Editor ranges are UTF-8 bytes; wire badges use JavaScript UTF-16 units.
use agentkit_protocol::generated as g;
use std::ops::Range;
/// Small, credential-free projection of a cached skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    /// Stable skill slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Searchable description.
    pub description: String,
    /// Workspace slug or upstream AGENTS_PLUGIN_NAME (.agents).
    pub plugin: String,
}
impl Skill {
    /// Project without cloning the skill's instructions.
    pub fn from_loaded(skill: &g::LoadedSkill, workspace: &str) -> Self {
        Self {
            slug: skill.slug.clone(),
            name: skill.metadata.name.clone(),
            description: skill.metadata.description.clone(),
            plugin: if skill.source == g::LoadedSkillSource::Workspace {
                workspace.into()
            } else {
                ".agents".into()
            },
        }
    }
}
/// Project only the catalog valid for the current session directory.
/// Shared by menu display and send-time metadata so stale project entries never leak.
pub fn catalog(
    loaded: &[g::LoadedSkill],
    workspace: &str,
    cached_directory: Option<&str>,
    effective_directory: Option<&str>,
) -> Vec<Skill> {
    loaded
        .iter()
        .filter(|skill| {
            skill.source != g::LoadedSkillSource::Project
                || (effective_directory.is_some() && cached_directory == effective_directory)
        })
        .map(|skill| Skill::from_loaded(skill, workspace))
        .collect()
}
/// Candidate source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// File search.
    File,
    /// Line-start skill search.
    Skill,
}
/// Current trigger and replacement range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    /// UTF-8 range including trigger.
    pub range: Range<usize>,
    /// Query without trigger.
    pub text: String,
    /// Search kind.
    pub kind: Kind,
}
/// Atomic editor token and menu row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Display name.
    pub label: String,
    /// Path or skill description.
    pub detail: String,
    /// Backing bracket text, or /compact.
    pub raw: String,
    /// Group: file, folder, skill, command.
    pub group: &'static str,
}
/// Detect at a collapsed cursor; never enter a completed bracket or an email address.
pub fn query(text: &str, cursor: usize) -> Option<Query> {
    let prefix = text.get(..cursor)?;
    let line = prefix.rsplit('\n').next()?;
    if let Some(at) = line.rfind('@') {
        let before = &line[..at];
        let tail = &line[at + 1..];
        if !tail.contains(['[', ']', '\r'])
            && !before.rsplit(']').next().unwrap_or_default().contains('[')
            && !before
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            return Some(Query {
                range: cursor - tail.len() - 1..cursor,
                text: tail.into(),
                kind: Kind::File,
            });
        }
    }
    let trimmed = line.trim_start_matches([' ', '\t']);
    let tail = trimmed.strip_prefix('/')?;
    if tail.contains(char::is_whitespace) || tail.contains(['[', ']']) {
        return None;
    }
    Some(Query {
        range: cursor - trimmed.len()..cursor,
        text: tail.into(),
        kind: Kind::Skill,
    })
}
/// Path candidates containing bracket delimiters cannot be represented by upstream grammar.
pub fn file_candidate(path: &str, folder: bool) -> Option<Candidate> {
    if path.is_empty() || path.contains([']', '\n', '\r']) {
        return None;
    }
    let group = if folder { "folder" } else { "file" };
    Some(Candidate {
        label: path.rsplit('/').next().unwrap_or(path).into(),
        detail: path.into(),
        raw: format!("[{group}:{path}]"),
        group,
    })
}
/// Match cached skills by name, slug and description; the built-in command stays a command.
pub fn skill_candidates(skills: &[Skill], query: &str) -> Vec<Candidate> {
    let needle = query.to_lowercase();
    skills
        .iter()
        .filter(|s| {
            format!("{} {} {}", s.slug, s.name, s.description)
                .to_lowercase()
                .contains(&needle)
        })
        .map(|s| Candidate {
            label: s.name.clone(),
            detail: s.description.clone(),
            raw: format!("[skill:{}:{}]", s.plugin, s.slug),
            group: "skill",
        })
        .collect()
}
fn skill_slug(body: &str) -> Option<&str> {
    let slug = if let Some((plugin, slug)) = body.rsplit_once(':') {
        if plugin.is_empty()
            || !plugin
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_ .-".contains(&c))
        {
            return None;
        }
        slug
    } else {
        body
    };
    (!slug.is_empty()
        && slug
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)))
    .then_some(slug)
}
/// Derive badges from the actual sent text, including manually entered bracket syntax.
pub fn metadata(text: &str, skills: &[Skill]) -> g::SendMessageOptions {
    let mut badges = Vec::new();
    let mut slugs = Vec::new();
    let mut consumed = [0; 3];
    for (start, _) in text.match_indices('[') {
        let Some(end) = text[start..].find(']').map(|n| start + n + 1) else {
            continue;
        };
        let raw = &text[start..end];
        let inside = &raw[1..raw.len() - 1];
        let Some((kind, value)) = inside.split_once(':') else {
            continue;
        };
        let index = match kind {
            "skill" => 0,
            "file" => 1,
            "folder" => 2,
            _ => continue,
        };
        if start < consumed[index] {
            continue;
        }
        let (type_, label, file_path, raw_text) = match kind {
            "skill" => {
                let Some(skill) =
                    skill_slug(value).and_then(|slug| skills.iter().find(|s| s.slug == slug))
                else {
                    continue;
                };
                if !slugs.contains(&skill.slug) {
                    slugs.push(skill.slug.clone());
                }
                (
                    g::SendMessageOptionsBadgesItemType::Skill,
                    if skill.name.is_empty() {
                        skill.slug.clone()
                    } else {
                        skill.name.clone()
                    },
                    None,
                    format!("[skill:{}:{}]", skill.plugin, skill.slug),
                )
            }
            "file" | "folder" if !value.is_empty() => (
                if kind == "file" {
                    g::SendMessageOptionsBadgesItemType::File
                } else {
                    g::SendMessageOptionsBadgesItemType::Folder
                },
                value
                    .rsplit('/')
                    .next()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(value)
                    .into(),
                Some(value.into()),
                raw.into(),
            ),
            _ => continue,
        };
        consumed[index] = end;
        badges.push(g::SendMessageOptionsBadgesItem {
            collapsed_label: None,
            end: text[..end].encode_utf16().count() as f64,
            file_path,
            icon_data_url: None,
            label,
            raw_text,
            start: text[..start].encode_utf16().count() as f64,
            type_,
        });
    }
    g::SendMessageOptions {
        badges: (!badges.is_empty()).then_some(badges),
        skill_slugs: (!slugs.is_empty()).then_some(slugs),
        ..Default::default()
    }
}
/// Validated UTF-16 wire span to a UTF-8 slice; rejects split surrogate pairs.
pub fn byte_range(text: &str, start: f64, end: f64) -> Option<Range<usize>> {
    if !start.is_finite()
        || !end.is_finite()
        || start.fract() != 0.
        || end.fract() != 0.
        || start < 0.
        || end < start
    {
        return None;
    }
    let mut units = 0;
    let mut a = None;
    let mut b = None;
    for (index, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if units as f64 == start {
            a = Some(index);
        }
        if units as f64 == end {
            b = Some(index);
            break;
        }
        units += c.len_utf16();
    }
    Some(a?..b?)
}
/// Badge projection for optimistic and transcript messages.
pub fn message_badges(options: &g::SendMessageOptions) -> Option<Vec<g::MessageBadgesItem>> {
    options.badges.as_ref().map(|items| {
        items
            .iter()
            .map(|b| g::MessageBadgesItem {
                collapsed_label: b.collapsed_label.clone(),
                end: b.end,
                file_path: b.file_path.clone(),
                icon_data_url: b.icon_data_url.clone(),
                label: b.label.clone(),
                raw_text: b.raw_text.clone(),
                start: b.start,
                type_: match b.type_ {
                    g::SendMessageOptionsBadgesItemType::Skill => g::MessageBadgesItemType::Skill,
                    g::SendMessageOptionsBadgesItemType::File => g::MessageBadgesItemType::File,
                    g::SendMessageOptionsBadgesItemType::Folder => g::MessageBadgesItemType::Folder,
                    g::SendMessageOptionsBadgesItemType::Context => {
                        g::MessageBadgesItemType::Context
                    }
                    g::SendMessageOptionsBadgesItemType::Command => {
                        g::MessageBadgesItemType::Command
                    }
                },
            })
            .collect()
    })
}

/// Upstream deduplicated extraction, including unresolved skill slugs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Known skills in occurrence order.
    pub skills: Vec<String>,
    /// Unresolved skill references in occurrence order.
    pub invalid_skills: Vec<String>,
    /// Unique file paths.
    pub files: Vec<String>,
    /// Unique folder paths.
    pub folders: Vec<String>,
}
/// Ported from mkagent v0.2.6 renderer/lib/mentions.ts parseMentions.
pub fn parse(text: &str, available: &[Skill]) -> Parsed {
    let mut parsed = Parsed::default();
    let mut consumed = [0; 3];
    for (start, _) in text.match_indices('[') {
        let Some(end) = text[start..].find(']').map(|n| start + n + 1) else {
            continue;
        };
        let Some((kind, value)) = text[start + 1..end - 1].split_once(':') else {
            continue;
        };
        let (index, id) = match kind {
            "skill" => {
                let Some(slug) = skill_slug(value) else {
                    continue;
                };
                (0, slug)
            }
            "file" if !value.is_empty() => (1, value),
            "folder" if !value.is_empty() => (2, value),
            _ => continue,
        };
        if start < consumed[index] {
            continue;
        }
        consumed[index] = end;
        let list = match index {
            0 if available.iter().any(|s| s.slug == id) => &mut parsed.skills,
            0 => &mut parsed.invalid_skills,
            1 => &mut parsed.files,
            _ => &mut parsed.folders,
        };
        if !list.iter().any(|item| item == id) {
            list.push(id.into());
        }
    }
    parsed
}
