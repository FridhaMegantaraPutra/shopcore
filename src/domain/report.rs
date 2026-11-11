use chrono::NaiveDate;
use serde::Serialize;

/// Order totals for one UTC calendar day, grouped by current status.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DailyReport {
    pub date: NaiveDate,
    pub order_count: i64,
    pub paid_cents: i64,
    pub cancelled_cents: i64,
    pub settled_cents: i64,
}
