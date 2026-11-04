//! Running a query against the project.

use std::cmp::Ordering;
use std::collections::HashMap;

use novo_core::{AnyId, Project};

use super::parse::{BinOp, Expr, Item, Query, QueryError, is_aggregate, parse};
use super::tables::{TABLES, Table, build};
use super::value::{QValue, compare, like};

/// The most rows a query may answer with (a runaway cross product cannot
/// happen with one table, but a page of a million rows is no report).
pub const MAX_ROWS: usize = 100_000;

/// A column of the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultColumn {
    /// The name (alias, column name or the text of the expression).
    pub name: String,
    /// The header to print.
    pub header: String,
}

/// What a query answered.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub table: String,
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Vec<QValue>>,
    /// The model objects behind each row; for a grouped row, every object
    /// of the group.
    pub sources: Vec<Vec<AnyId>>,
}

/// Reads and runs a query on the project.
pub fn run(project: &Project, sql: &str) -> Result<QueryResult, QueryError> {
    let query = parse(sql)?;
    run_query(project, &query)
}

/// Checks a query without a project to run it on: syntax, table and
/// column names, functions. Returns the tables the project has.
pub fn check(project: &Project, sql: &str) -> Result<Query, QueryError> {
    let query = parse(sql)?;
    let table = build(project, &query.table).ok_or_else(|| unknown_table(&query))?;
    Bound::new(&query, &table)?;
    Ok(query)
}

fn unknown_table(q: &Query) -> QueryError {
    let names: Vec<&str> = TABLES.iter().map(|(n, _)| *n).collect();
    let hint = closest(&q.table, names.iter().copied())
        .map(|n| format!(" Did you mean `{n}`?"))
        .unwrap_or_default();
    QueryError::new(
        format!(
            "there is no table `{}`.{hint} Tables: {}",
            q.table,
            names.join(", ")
        ),
        q.table_at,
    )
}

/// The candidate closest to `word` (an edit distance of 2 or less, or a prefix).
fn closest<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let word = word.to_lowercase();
    candidates
        .map(|c| (distance(&word, &c.to_lowercase()), c))
        .filter(|(d, c)| *d <= 2 || c.to_lowercase().starts_with(&word))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let cur = row[j];
            row[j] = (prev + usize::from(a[i - 1] != b[j - 1]))
                .min(row[j] + 1)
                .min(row[j - 1] + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// How a sort key is found for an answer row.
enum Key {
    /// A column of the answer.
    Output(usize),
    /// An expression of the source row (or group).
    Expr(Expr),
}

/// A query with its names resolved against a table.
struct Bound {
    columns: Vec<(ResultColumn, Expr)>,
    filter: Option<Expr>,
    group_by: Vec<Expr>,
    having: Option<Expr>,
    order: Vec<(Key, bool)>,
    grouped: bool,
}

impl Bound {
    fn new(q: &Query, table: &Table) -> Result<Bound, QueryError> {
        let bind = |e: &Expr| bind(e, table);
        let mut columns: Vec<(ResultColumn, Expr)> = Vec::new();
        for item in &q.items {
            match item {
                Item::Star => {
                    for (i, c) in table.columns.iter().enumerate() {
                        columns.push((
                            ResultColumn {
                                name: c.name.clone(),
                                header: c.header.clone(),
                            },
                            Expr::Col {
                                name: c.name.clone(),
                                at: 0,
                                index: Some(i),
                            },
                        ));
                    }
                }
                Item::Expr { expr, alias, text } => {
                    let bound = bind(expr)?;
                    let (name, header) = match (alias, &bound) {
                        (Some(a), _) => (a.clone(), a.clone()),
                        (None, Expr::Col { index: Some(i), .. }) => (
                            table.columns[*i].name.clone(),
                            table.columns[*i].header.clone(),
                        ),
                        _ => (text.clone(), text.clone()),
                    };
                    columns.push((ResultColumn { name, header }, bound));
                }
            }
        }
        let filter = q.filter.as_ref().map(bind).transpose()?;
        if let Some(f) = &filter
            && f.has_aggregate()
        {
            return Err(QueryError::new(
                "a total (count, sum, ...) cannot be used in WHERE; use HAVING after GROUP BY",
                q.table_at,
            ));
        }
        let group_by: Vec<Expr> = q.group_by.iter().map(bind).collect::<Result<_, _>>()?;
        if group_by.iter().any(Expr::has_aggregate) {
            return Err(QueryError::new(
                "a total (count, sum, ...) cannot be used in GROUP BY",
                q.table_at,
            ));
        }
        let having = q.having.as_ref().map(bind).transpose()?;
        let grouped = !group_by.is_empty()
            || having.is_some()
            || columns.iter().any(|(_, e)| e.has_aggregate());
        let mut order = Vec::new();
        for key in &q.order_by {
            // A number picks the nth column; a name that is a column of the
            // answer (an alias) picks that column.
            if let Expr::Lit(QValue::Int(n)) = &key.expr {
                let n = usize::try_from(*n).unwrap_or(0);
                if n == 0 || n > columns.len() {
                    return Err(QueryError::new(
                        format!("ORDER BY {n}: the answer has {} columns", columns.len()),
                        key.at,
                    ));
                }
                order.push((Key::Output(n - 1), key.descending));
                continue;
            }
            if let Expr::Col { name, .. } = &key.expr
                && let Some(i) = columns
                    .iter()
                    .position(|(c, _)| c.name.eq_ignore_ascii_case(name))
                && (table.column(name).is_none()
                    || matches!(&columns[i].1, Expr::Col { name: n, .. } if n.eq_ignore_ascii_case(name)))
            {
                order.push((Key::Output(i), key.descending));
                continue;
            }
            let bound = bind(&key.expr)?;
            match columns.iter().position(|(_, e)| *e == bound) {
                Some(i) => order.push((Key::Output(i), key.descending)),
                None => order.push((Key::Expr(bound), key.descending)),
            }
        }
        let all = columns
            .iter()
            .map(|(_, e)| e)
            .chain(filter.iter())
            .chain(group_by.iter())
            .chain(having.iter());
        for e in all {
            check_functions(e)?;
        }
        for (key, _) in &order {
            if let Key::Expr(e) = key {
                check_functions(e)?;
            }
        }
        Ok(Bound {
            columns,
            filter,
            group_by,
            having,
            order,
            grouped,
        })
    }
}

/// Fills in the column positions of an expression.
fn bind(expr: &Expr, table: &Table) -> Result<Expr, QueryError> {
    let rec = |e: &Expr| bind(e, table).map(Box::new);
    Ok(match expr {
        Expr::Lit(v) => Expr::Lit(v.clone()),
        Expr::Col { name, at, .. } => {
            let bare = name.split_once('.').map_or(name.as_str(), |(t, c)| {
                if t.eq_ignore_ascii_case(table.name) {
                    c
                } else {
                    name.as_str()
                }
            });
            let index = table.column(bare).ok_or_else(|| {
                let hint = closest(bare, table.columns.iter().map(|c| c.name.as_str()))
                    .map(|n| format!(" Did you mean `{n}`?"))
                    .unwrap_or_default();
                let names: Vec<&str> = table
                    .columns
                    .iter()
                    .filter(|c| !c.name.starts_with("prop_"))
                    .map(|c| c.name.as_str())
                    .collect();
                QueryError::new(
                    format!(
                        "`{table_name}` has no column `{name}`.{hint} Columns: {}",
                        names.join(", "),
                        table_name = table.name
                    ),
                    *at,
                )
            })?;
            Expr::Col {
                name: table.columns[index].name.clone(),
                at: *at,
                index: Some(index),
            }
        }
        Expr::Not(e) => Expr::Not(rec(e)?),
        Expr::Neg(e) => Expr::Neg(rec(e)?),
        Expr::Bin(op, a, b) => Expr::Bin(*op, rec(a)?, rec(b)?),
        Expr::IsNull(e, n) => Expr::IsNull(rec(e)?, *n),
        Expr::Like(a, b, n) => Expr::Like(rec(a)?, rec(b)?, *n),
        Expr::In(e, list, n) => Expr::In(
            rec(e)?,
            list.iter()
                .map(|x| bind(x, table))
                .collect::<Result<_, _>>()?,
            *n,
        ),
        Expr::Between(a, b, c, n) => Expr::Between(rec(a)?, rec(b)?, rec(c)?, *n),
        Expr::Func {
            name,
            args,
            distinct,
            star,
            at,
        } => Expr::Func {
            name: name.clone(),
            args: args
                .iter()
                .map(|x| bind(x, table))
                .collect::<Result<_, _>>()?,
            distinct: *distinct,
            star: *star,
            at: *at,
        },
    })
}

/// `(name, fewest arguments, most arguments)`.
const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("lower", 1, 1),
    ("upper", 1, 1),
    ("length", 1, 1),
    ("trim", 1, 1),
    ("abs", 1, 1),
    ("round", 1, 2),
    ("coalesce", 1, 16),
    ("ifnull", 2, 2),
    ("substr", 2, 3),
    ("replace", 3, 3),
    ("concat", 1, 16),
    ("count", 1, 1),
    ("sum", 1, 1),
    ("avg", 1, 1),
    ("min", 1, 1),
    ("max", 1, 1),
    ("group_concat", 1, 2),
];

fn check_functions(expr: &Expr) -> Result<(), QueryError> {
    let mut error = None;
    expr.walk(&mut |e| {
        if error.is_some() {
            return;
        }
        if let Expr::Func {
            name,
            args,
            star,
            at,
            distinct,
        } = e
        {
            let Some((_, low, high)) = FUNCTIONS.iter().find(|(n, _, _)| n == name) else {
                let hint = closest(name, FUNCTIONS.iter().map(|(n, _, _)| *n))
                    .map(|n| format!(" Did you mean `{n}`?"))
                    .unwrap_or_default();
                error = Some(QueryError::new(
                    format!("there is no function `{name}`.{hint}"),
                    *at,
                ));
                return;
            };
            if *star && name != "count" {
                error = Some(QueryError::new(
                    format!("`{name}(*)` is not allowed; only count(*) takes *"),
                    *at,
                ));
            } else if !*star && !(*low..=*high).contains(&args.len()) {
                error = Some(QueryError::new(
                    format!("`{name}` takes {low} to {high} values, not {}", args.len()),
                    *at,
                ));
            } else if *distinct && !is_aggregate(name) {
                error = Some(QueryError::new(
                    format!("DISTINCT only goes with totals, not `{name}`"),
                    *at,
                ));
            }
        }
    });
    error.map_or(Ok(()), Err)
}

/// Where an expression looks for its values.
struct Ctx<'a> {
    rows: &'a [Vec<QValue>],
    /// The row columns read from (the first of a group).
    current: Option<usize>,
    /// The rows a total adds up; `None` outside a group.
    group: Option<&'a [usize]>,
}

fn eval(expr: &Expr, ctx: &Ctx<'_>) -> QValue {
    use QValue::{Bool, Null};
    let truth = |v: &QValue| if v.is_null() { None } else { Some(v.truthy()) };
    match expr {
        Expr::Lit(v) => v.clone(),
        Expr::Col { index, .. } => match (ctx.current, index) {
            (Some(r), Some(i)) => ctx.rows[r][*i].clone(),
            _ => Null,
        },
        Expr::Not(e) => match truth(&eval(e, ctx)) {
            Some(b) => Bool(!b),
            None => Null,
        },
        Expr::Neg(e) => match eval(e, ctx) {
            QValue::Int(i) => QValue::Int(-i),
            other => other.as_f64().map_or(Null, |n| QValue::Num(-n)),
        },
        Expr::Bin(op, a, b) => {
            let (x, y) = (eval(a, ctx), eval(b, ctx));
            match op {
                BinOp::And => match (truth(&x), truth(&y)) {
                    (Some(false), _) | (_, Some(false)) => Bool(false),
                    (Some(true), Some(true)) => Bool(true),
                    _ => Null,
                },
                BinOp::Or => match (truth(&x), truth(&y)) {
                    (Some(true), _) | (_, Some(true)) => Bool(true),
                    (Some(false), Some(false)) => Bool(false),
                    _ => Null,
                },
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    match compare(&x, &y) {
                        None => Null,
                        Some(o) => Bool(match op {
                            BinOp::Eq => o == Ordering::Equal,
                            BinOp::Ne => o != Ordering::Equal,
                            BinOp::Lt => o == Ordering::Less,
                            BinOp::Le => o != Ordering::Greater,
                            BinOp::Gt => o == Ordering::Greater,
                            _ => o != Ordering::Less,
                        }),
                    }
                }
                BinOp::Concat => {
                    if x.is_null() || y.is_null() {
                        Null
                    } else {
                        QValue::Text(format!("{}{}", x.display(), y.display()))
                    }
                }
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => arithmetic(*op, &x, &y),
            }
        }
        Expr::IsNull(e, negated) => Bool(eval(e, ctx).is_null() != *negated),
        Expr::Like(a, p, negated) => {
            let (x, p) = (eval(a, ctx), eval(p, ctx));
            if x.is_null() || p.is_null() {
                Null
            } else {
                Bool(like(&x.display(), &p.display()) != *negated)
            }
        }
        Expr::In(e, list, negated) => {
            let x = eval(e, ctx);
            if x.is_null() {
                return Null;
            }
            let hit = list
                .iter()
                .any(|v| compare(&x, &eval(v, ctx)) == Some(Ordering::Equal));
            Bool(hit != *negated)
        }
        Expr::Between(e, low, high, negated) => {
            let (x, lo, hi) = (eval(e, ctx), eval(low, ctx), eval(high, ctx));
            match (compare(&x, &lo), compare(&x, &hi)) {
                (Some(a), Some(b)) => {
                    Bool((a != Ordering::Less && b != Ordering::Greater) != *negated)
                }
                _ => Null,
            }
        }
        Expr::Func {
            name,
            args,
            distinct,
            star,
            ..
        } => {
            if is_aggregate(name) {
                return aggregate(name, args, *distinct, *star, ctx);
            }
            let v: Vec<QValue> = args.iter().map(|a| eval(a, ctx)).collect();
            scalar(name, &v)
        }
    }
}

fn arithmetic(op: BinOp, x: &QValue, y: &QValue) -> QValue {
    if let (QValue::Int(a), QValue::Int(b)) = (x, y) {
        let exact = match op {
            BinOp::Add => a.checked_add(*b),
            BinOp::Sub => a.checked_sub(*b),
            BinOp::Mul => a.checked_mul(*b),
            _ if *b != 0 && a % b == 0 => a.checked_div(*b),
            _ => None,
        };
        if let Some(n) = exact {
            return QValue::Int(n);
        }
    }
    let (Some(a), Some(b)) = (x.as_f64(), y.as_f64()) else {
        return QValue::Null;
    };
    let n = match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        _ if b == 0.0 => return QValue::Null,
        _ => a / b,
    };
    QValue::Num(n)
}

fn scalar(name: &str, v: &[QValue]) -> QValue {
    let text = |i: usize| v.get(i).filter(|x| !x.is_null()).map(QValue::display);
    match name {
        "lower" => text(0).map_or(QValue::Null, |t| QValue::text(t.to_lowercase())),
        "upper" => text(0).map_or(QValue::Null, |t| QValue::text(t.to_uppercase())),
        "trim" => text(0).map_or(QValue::Null, |t| QValue::text(t.trim())),
        "length" => text(0).map_or(QValue::Null, |t| QValue::from(t.chars().count())),
        "abs" => match &v[0] {
            QValue::Int(i) => QValue::Int(i.abs()),
            other => other
                .as_f64()
                .map_or(QValue::Null, |n| QValue::Num(n.abs())),
        },
        "round" => {
            let digits = v.get(1).and_then(QValue::as_f64).unwrap_or(0.0) as i32;
            v[0].as_f64().map_or(QValue::Null, |n| {
                let k = 10f64.powi(digits.clamp(0, 9));
                QValue::Num((n * k).round() / k)
            })
        }
        "coalesce" | "ifnull" => v
            .iter()
            .find(|x| !x.is_null())
            .cloned()
            .unwrap_or(QValue::Null),
        "substr" => {
            let Some(t) = text(0) else {
                return QValue::Null;
            };
            let start = v[1].as_f64().unwrap_or(1.0).max(1.0) as usize - 1;
            let len = v
                .get(2)
                .and_then(QValue::as_f64)
                .map_or(usize::MAX, |l| l.max(0.0) as usize);
            QValue::text(t.chars().skip(start).take(len).collect::<String>())
        }
        "replace" => match (text(0), text(1), text(2)) {
            (Some(t), Some(from), Some(to)) if !from.is_empty() => {
                QValue::text(t.replace(&from, &to))
            }
            (Some(t), _, _) => QValue::text(t),
            _ => QValue::Null,
        },
        "concat" => QValue::text(
            v.iter()
                .filter(|x| !x.is_null())
                .map(QValue::display)
                .collect::<String>(),
        ),
        _ => QValue::Null,
    }
}

fn aggregate(name: &str, args: &[Expr], distinct: bool, star: bool, ctx: &Ctx<'_>) -> QValue {
    let group = ctx.group.unwrap_or(&[]);
    if star {
        return QValue::from(group.len());
    }
    let mut values: Vec<QValue> = group
        .iter()
        .map(|&r| {
            eval(
                &args[0],
                &Ctx {
                    rows: ctx.rows,
                    current: Some(r),
                    group: None,
                },
            )
        })
        .filter(|v| !v.is_null())
        .collect();
    if distinct {
        let mut seen = std::collections::HashSet::new();
        values.retain(|v| seen.insert(format!("{v:?}")));
    }
    match name {
        "count" => QValue::from(values.len()),
        "sum" | "avg" => {
            if values.is_empty() {
                return QValue::Null;
            }
            let numbers: Vec<f64> = values.iter().filter_map(QValue::as_f64).collect();
            let total: f64 = numbers.iter().sum();
            if name == "avg" {
                QValue::Num(total / numbers.len().max(1) as f64)
            } else if values.iter().all(|v| matches!(v, QValue::Int(_))) {
                QValue::Int(total as i64)
            } else {
                QValue::Num(total)
            }
        }
        "min" | "max" => {
            let want = if name == "min" {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            values
                .into_iter()
                .reduce(|best, v| {
                    if compare(&v, &best) == Some(want) {
                        v
                    } else {
                        best
                    }
                })
                .unwrap_or(QValue::Null)
        }
        _ => {
            let separator = args
                .get(1)
                .map(|s| {
                    eval(
                        s,
                        &Ctx {
                            rows: ctx.rows,
                            current: None,
                            group: None,
                        },
                    )
                    .display()
                })
                .unwrap_or_else(|| ", ".to_owned());
            if values.is_empty() {
                QValue::Null
            } else {
                QValue::Text(
                    values
                        .iter()
                        .map(QValue::display)
                        .collect::<Vec<_>>()
                        .join(&separator),
                )
            }
        }
    }
}

/// Runs a parsed query.
pub fn run_query(project: &Project, q: &Query) -> Result<QueryResult, QueryError> {
    let table = build(project, &q.table).ok_or_else(|| unknown_table(q))?;
    let bound = Bound::new(q, &table)?;
    let rows = &table.rows;

    let single = |r: usize| Ctx {
        rows,
        current: Some(r),
        group: None,
    };
    let kept: Vec<usize> = (0..rows.len())
        .filter(|&r| {
            bound
                .filter
                .as_ref()
                .is_none_or(|f| eval(f, &single(r)).truthy())
        })
        .collect();

    // Each answer row, its sort keys and the objects behind it.
    struct Out {
        values: Vec<QValue>,
        keys: Vec<QValue>,
        sources: Vec<AnyId>,
    }
    let order_keys = |ctx: &Ctx<'_>, values: &[QValue]| -> Vec<QValue> {
        bound
            .order
            .iter()
            .map(|(key, _)| match key {
                Key::Output(i) => values[*i].clone(),
                Key::Expr(e) => eval(e, ctx),
            })
            .collect()
    };
    let mut out: Vec<Out> = Vec::new();
    if bound.grouped {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for &r in &kept {
            let key = bound
                .group_by
                .iter()
                .map(|g| format!("{:?}", eval(g, &single(r))))
                .collect::<Vec<_>>()
                .join("\u{1f}");
            groups
                .entry(key.clone())
                .or_insert_with(|| {
                    order.push(key);
                    Vec::new()
                })
                .push(r);
        }
        // A total over no rows is still one row (`count(*)` = 0).
        if bound.group_by.is_empty() && order.is_empty() {
            order.push(String::new());
            groups.insert(String::new(), Vec::new());
        }
        for key in order {
            let members = &groups[&key];
            let ctx = Ctx {
                rows,
                current: members.first().copied(),
                group: Some(members),
            };
            if bound
                .having
                .as_ref()
                .is_some_and(|h| !eval(h, &ctx).truthy())
            {
                continue;
            }
            let values: Vec<QValue> = bound.columns.iter().map(|(_, e)| eval(e, &ctx)).collect();
            let keys = order_keys(&ctx, &values);
            out.push(Out {
                values,
                keys,
                sources: members
                    .iter()
                    .flat_map(|&r| table.sources[r].iter().copied())
                    .collect(),
            });
        }
    } else {
        for &r in &kept {
            let ctx = single(r);
            let values: Vec<QValue> = bound.columns.iter().map(|(_, e)| eval(e, &ctx)).collect();
            let keys = order_keys(&ctx, &values);
            out.push(Out {
                values,
                keys,
                sources: table.sources[r].clone(),
            });
        }
    }

    if q.distinct {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut unique: Vec<Out> = Vec::new();
        for row in out {
            let key = format!("{:?}", row.values);
            match seen.get(&key) {
                Some(&i) => unique[i].sources.extend(row.sources),
                None => {
                    seen.insert(key, unique.len());
                    unique.push(row);
                }
            }
        }
        out = unique;
    }

    if !bound.order.is_empty() {
        out.sort_by(|a, b| {
            for (i, (_, descending)) in bound.order.iter().enumerate() {
                let (x, y) = (&a.keys[i], &b.keys[i]);
                // An empty value sorts last, whichever way the column goes.
                let ord = match (x.is_null(), y.is_null()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => return Ordering::Greater,
                    (false, true) => return Ordering::Less,
                    _ => compare(x, y).unwrap_or(Ordering::Equal),
                };
                let ord = if *descending { ord.reverse() } else { ord };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });
    }
    out.truncate(q.limit.unwrap_or(MAX_ROWS).min(MAX_ROWS));

    Ok(QueryResult {
        table: table.name.to_owned(),
        columns: bound.columns.into_iter().map(|(c, _)| c).collect(),
        rows: out.iter().map(|o| o.values.clone()).collect(),
        sources: out.into_iter().map(|o| o.sources).collect(),
    })
}
