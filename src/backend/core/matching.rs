// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

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
