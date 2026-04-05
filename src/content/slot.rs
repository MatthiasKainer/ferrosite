use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The complete taxonomy of slots an article can target.
///
/// Slots follow Atomic Design hierarchy:
///   Atoms → Molecules → Organisms → Regions (layout zones)
///
/// Each article's frontmatter declares which slot it targets via `slot = "..."`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SlotType {
    // ── ATOMS ────────────────────────────────────────────────────────────
    /// Plain text or HTML snippet
    TextBlock,
    /// Single image with optional caption and alt
    Image,
    /// Skill / technology / status badge
    Badge,
    /// Call-to-action link button
    LinkButton,
    /// Syntax-highlighted code snippet
    CodeSnippet,
    /// Pull-quote or blockquote highlight
    PullQuote,
    /// Stat number with label (e.g. "5+ years", "42 projects")
    StatNumber,

    // ── MOLECULES ────────────────────────────────────────────────────────
    /// Blog post card (title, date, excerpt, tags, link)
    ArticleCard,
    /// Portfolio project card (title, stack, description, links)
    ProjectCard,
    /// Group of related skill/tech badges
    SkillGroup,
    /// Social media profile link
    SocialLink,
    /// Single timeline entry (role, company, dates, description)
    TimelineEntry,
    /// Navigation link with optional icon
    NavItem,
    /// Dock shortcut item (icon, label, url)
    DockItem,
    /// Testimonial / recommendation (author, role, quote)
    Testimonial,
    /// Featured highlight card (icon, title, description)
    FeatureCard,
    /// File/resource download item
    DownloadItem,

    // ── ORGANISMS ────────────────────────────────────────────────────────
    /// Page hero (headline, sub-headline, CTA, background)
    Hero,
    /// Paginated list/feed of blog articles
    BlogFeed,
    /// Grid of portfolio projects
    ProjectGrid,
    /// Skills overview with categories and levels
    SkillsMatrix,
    /// Career/education chronological timeline
    CareerTimeline,
    /// Contact form (always plugin-driven, dynamic)
    ContactForm,
    /// Testimonials carousel / grid
    TestimonialSection,
    /// Key statistics / numbers section
    StatsBar,
    /// Open source contributions / GitHub activity
    OpenSourceSection,
    /// Speaking / conference appearances
    SpeakingSection,
    /// Newsletter / email signup
    NewsletterSignup,
    /// Page-level SEO/structured-data block (not rendered visually)
    SeoMeta,

    // ── LAYOUT REGIONS ───────────────────────────────────────────────────
    /// Site-wide header branding / site title / logo
    HeaderBrand,
    /// Header action area (buttons, search, CTA)
    HeaderAction,
    /// Footer about / copyright text
    FooterAbout,
    /// Footer navigation column
    FooterNavColumn,
    /// Footer bottom bar (legal, credits)
    FooterBottom,
    /// Sidebar widget (arbitrary small unit)
    SidebarWidget,
    /// Table of contents (auto-generated or manual)
    TableOfContents,

    // ── PAGE-LEVEL META ──────────────────────────────────────────────────
    /// Full blog/article body (rendered from markdown body)
    ArticleBody,
    /// Full project details body
    ProjectBody,
    /// About page full bio
    AboutBody,
}

impl SlotType {
    /// Human-readable display name for templates and error messages.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::TextBlock => "Text Block",
            Self::Image => "Image",
            Self::Badge => "Badge",
            Self::LinkButton => "Link Button",
            Self::CodeSnippet => "Code Snippet",
            Self::PullQuote => "Pull Quote",
            Self::StatNumber => "Stat Number",
            Self::ArticleCard => "Article Card",
            Self::ProjectCard => "Project Card",
            Self::SkillGroup => "Skill Group",
            Self::SocialLink => "Social Link",
            Self::TimelineEntry => "Timeline Entry",
            Self::NavItem => "Nav Item",
            Self::DockItem => "Dock Item",
            Self::Testimonial => "Testimonial",
            Self::FeatureCard => "Feature Card",
            Self::DownloadItem => "Download Item",
            Self::Hero => "Hero Section",
            Self::BlogFeed => "Blog Feed",
            Self::ProjectGrid => "Project Grid",
            Self::SkillsMatrix => "Skills Matrix",
            Self::CareerTimeline => "Career Timeline",
            Self::ContactForm => "Contact Form",
            Self::TestimonialSection => "Testimonial Section",
            Self::StatsBar => "Stats Bar",
            Self::OpenSourceSection => "Open Source Section",
            Self::SpeakingSection => "Speaking Section",
            Self::NewsletterSignup => "Newsletter Signup",
            Self::SeoMeta => "SEO Meta",
            Self::HeaderBrand => "Header Brand",
            Self::HeaderAction => "Header Action",
            Self::FooterAbout => "Footer About",
            Self::FooterNavColumn => "Footer Nav Column",
            Self::FooterBottom => "Footer Bottom",
            Self::SidebarWidget => "Sidebar Widget",
            Self::TableOfContents => "Table of Contents",
            Self::ArticleBody => "Article Body",
            Self::ProjectBody => "Project Body",
            Self::AboutBody => "About Body",
        }
    }

    /// The Atomic Design tier this slot belongs to.
    pub fn tier(&self) -> SlotTier {
        match self {
            Self::TextBlock
            | Self::Image
            | Self::Badge
            | Self::LinkButton
            | Self::CodeSnippet
            | Self::PullQuote
            | Self::StatNumber => SlotTier::Atom,

            Self::ArticleCard
            | Self::ProjectCard
            | Self::SkillGroup
            | Self::SocialLink
            | Self::TimelineEntry
            | Self::NavItem
            | Self::DockItem
            | Self::Testimonial
            | Self::FeatureCard
            | Self::DownloadItem => SlotTier::Molecule,

            Self::Hero
            | Self::BlogFeed
            | Self::ProjectGrid
            | Self::SkillsMatrix
            | Self::CareerTimeline
            | Self::ContactForm
            | Self::TestimonialSection
            | Self::StatsBar
            | Self::OpenSourceSection
            | Self::SpeakingSection
            | Self::NewsletterSignup
            | Self::SeoMeta => SlotTier::Organism,

            Self::HeaderBrand
            | Self::HeaderAction
            | Self::FooterAbout
            | Self::FooterNavColumn
            | Self::FooterBottom
            | Self::SidebarWidget
            | Self::TableOfContents
            | Self::ArticleBody
            | Self::ProjectBody
            | Self::AboutBody => SlotTier::Region,
        }
    }

    /// Which page types this slot can appear on.
    pub fn allowed_page_types(&self) -> &'static [&'static str] {
        match self {
            Self::Hero => &["home", "about", "contact"],
            Self::BlogFeed => &["home", "blog"],
            Self::ArticleCard => &["home", "blog"],
            Self::ArticleBody => &["post"],
            Self::ProjectGrid => &["home", "projects"],
            Self::ProjectCard => &["home", "projects"],
            Self::ProjectBody => &["project"],
            Self::SkillsMatrix | Self::SkillGroup | Self::Badge => &["home", "about"],
            Self::CareerTimeline | Self::TimelineEntry => &["about"],
            Self::AboutBody => &["about"],
            Self::ContactForm => &["contact"],
            Self::SeoMeta => &["*"],
            Self::NavItem => &["*"],
            Self::DockItem => &["*"],
            Self::SidebarWidget => &["*"],
            Self::HeaderBrand | Self::HeaderAction => &["*"],
            Self::FooterAbout | Self::FooterNavColumn | Self::FooterBottom => &["*"],
            Self::SocialLink => &["home", "about", "contact", "*"],
            Self::TestimonialSection | Self::Testimonial => &["home", "about"],
            Self::StatsBar | Self::StatNumber => &["home", "about"],
            Self::OpenSourceSection => &["home", "about", "projects"],
            Self::SpeakingSection => &["about"],
            Self::NewsletterSignup => &["home", "blog", "post"],
            Self::TableOfContents => &["post", "project"],
            Self::FeatureCard => &["home", "about"],
            Self::TextBlock => &["*"],
            Self::Image => &["*"],
            Self::LinkButton => &["*"],
            Self::CodeSnippet => &["post", "project"],
            Self::PullQuote => &["post", "about"],
            Self::DownloadItem => &["post", "project", "about"],
        }
    }

    /// Whether multiple articles can target this slot (vs single-occupancy).
    pub fn is_multi(&self) -> bool {
        matches!(
            self,
            Self::ArticleCard
                | Self::NavItem
                | Self::DockItem
                | Self::SocialLink
                | Self::TimelineEntry
                | Self::Badge
                | Self::SkillGroup
                | Self::Testimonial
                | Self::FeatureCard
                | Self::FooterNavColumn
                | Self::SidebarWidget
                | Self::StatNumber
                | Self::DownloadItem
                | Self::ProjectCard
                | Self::CodeSnippet
                | Self::TextBlock
                | Self::Image
                | Self::LinkButton
        )
    }
}

impl FromStr for SlotType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|_| ())
    }
}

impl fmt::Display for SlotType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Round-trip through serde for canonical kebab-case
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| format!("{:?}", self));
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotTier {
    Atom,
    Molecule,
    Organism,
    Region,
}

impl fmt::Display for SlotTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atom => write!(f, "Atom"),
            Self::Molecule => write!(f, "Molecule"),
            Self::Organism => write!(f, "Organism"),
            Self::Region => write!(f, "Region"),
        }
    }
}

/// A slot assignment — which slot an article fills, with ordering metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAssignment {
    pub slot_type: SlotType,
    /// Ordering within the slot (lower = earlier)
    #[serde(default)]
    pub order: i32,
    /// Weight/priority for ranking (higher = more prominent)
    #[serde(default = "default_weight")]
    pub weight: i32,
    /// Which page this assignment applies to ("*" = all pages)
    #[serde(default = "default_page_scope")]
    pub page_scope: String,
}

fn default_weight() -> i32 {
    50
}
fn default_page_scope() -> String {
    "*".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_type_round_trips_through_kebab_case() {
        let slot: SlotType = "article-card".parse().expect("slot should parse");

        assert_eq!(slot, SlotType::ArticleCard);
        assert_eq!(slot.to_string(), "article-card");
    }

    #[test]
    fn slot_metadata_matches_template_intent() {
        assert_eq!(SlotType::Hero.display_name(), "Hero Section");
        assert_eq!(SlotType::Hero.tier(), SlotTier::Organism);
        assert_eq!(
            SlotType::Hero.allowed_page_types(),
            &["home", "about", "contact"]
        );
        assert!(!SlotType::Hero.is_multi());

        assert_eq!(SlotType::NavItem.tier(), SlotTier::Molecule);
        assert_eq!(SlotType::NavItem.allowed_page_types(), &["*"]);
        assert!(SlotType::NavItem.is_multi());
    }
}
