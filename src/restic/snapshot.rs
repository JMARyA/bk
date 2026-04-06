use facet::Facet;

use crate::restic::ResticSummaryMsg;

#[derive(Facet, Debug)]
#[facet(deny_unknown_fields)]
pub struct Snapshot {
    pub time: String,
    pub tree: String,
    pub paths: Vec<String>,
    pub hostname: String,
    pub username: String,
    pub parent: Option<String>,
    pub uid: Option<i64>,
    pub gid: Option<i64>,
    #[facet(default)]
    pub tags: Vec<String>,
    pub original: Option<String>,
    pub program_version: String,
    pub summary: ResticSummaryMsg,
    pub id: String,
    pub short_id: String,
}
