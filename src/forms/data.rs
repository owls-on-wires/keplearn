#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub names: Vec<String>,
    pub cols: Vec<Vec<f64>>,
    pub y: Vec<f64>,
    pub target_name: String,
}

impl Table {}
