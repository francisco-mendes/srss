use std::{
    borrow::Cow,
    fmt::{
        Display,
        Formatter,
    },
    hash::{
        Hash,
        Hasher,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub station: Station,
    pub records: Vec<Record>,
}

#[derive(Debug, Clone, Eq)]
pub struct Station {
    pub id: String,
    pub name: String,
}

impl PartialEq for Station {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.id, &other.id)
    }
}

impl Hash for Station {
    fn hash<H: Hasher>(&self, state: &mut H) {
        String::hash(&self.id, state)
    }
}

impl Display for Station {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.name, f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub date: String,
    pub pv_yield: Option<f64>,
}

impl Display for Record {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        try {
            write!(f, "[")?;
            Display::fmt(&self.date, f)?;
            write!(f, "]: ")?;
            f.write_str(&self.fmt_yield())?;
        }
    }
}

impl Record {
    fn fmt_yield(&self) -> Cow<'static, str> {
        self.pv_yield
            .map_or(Cow::from("0"), |v| v.to_string().into())
    }

    pub fn to_value(&self) -> String {
        self.fmt_yield().into_owned()
    }

    pub fn to_csv(&self) -> String {
        format!("{}; {}", self.date, self.fmt_yield())
    }
}
