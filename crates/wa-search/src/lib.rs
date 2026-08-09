//! Provider-neutral web search with native SearXNG and Degoog adapters.

mod client;
mod model;
mod providers;

pub use client::{SearchClient, SearchProviderConfig};
pub use model::{SearchDegradationWarning, SearchResponse, UnavailableUpstream};
