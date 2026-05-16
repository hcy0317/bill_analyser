use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::import_learning::{build_composite_match_features, normalize_import_learning_text};
use crate::primitives::parse_bill_datetime;

include!("matching/types_and_ids.rs");
include!("matching/session_learning.rs");
include!("matching/investment.rs");
include!("matching/recurring.rs");
include!("matching/session_serialization.rs");
include!("matching/similarity_helpers.rs");
