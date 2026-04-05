pub mod article;
pub mod frontmatter;
pub mod page;
pub mod slot;

pub use article::Article;
pub use frontmatter::{parse_document, Frontmatter};
pub use page::{Page, PageCollection, PageType, SlotMap};
pub use slot::{SlotAssignment, SlotTier, SlotType};
