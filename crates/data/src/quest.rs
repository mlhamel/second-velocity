use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct QuestDef {
    pub id: String,
    pub name: String,
    pub giver: (String, String),
    pub kind: QuestKind,
    pub reward_credits: u32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum QuestKind {
    Delivery {
        commodity: String,
        qty: u32,
        destination: (String, String),
        #[serde(default)]
        deadline_secs: Option<u64>,
    },
    Bounty {
        system: String,
        target: String,
        #[serde(default)]
        deadline_secs: Option<u64>,
    },
}

impl QuestKind {
    pub fn deadline_secs(&self) -> Option<u64> {
        match self {
            Self::Delivery { deadline_secs, .. } | Self::Bounty { deadline_secs, .. } => {
                *deadline_secs
            }
        }
    }
}
