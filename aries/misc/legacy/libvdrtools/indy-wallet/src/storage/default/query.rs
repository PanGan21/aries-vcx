use std::convert::From;

use indy_api_types::errors::prelude::*;

use crate::language::{Operator, TagName, TargetValue};

#[derive(Debug)]
pub(crate) enum ToSQL<'a> {
    ByteSlice(&'a [u8]),
    CharSlice(&'a str),
}

impl<'a> From<&'a Vec<u8>> for ToSQL<'a> {
    fn from(item: &'a Vec<u8>) -> Self {
        ToSQL::ByteSlice(item.as_slice())
    }
}

impl<'a> From<&'a [u8]> for ToSQL<'a> {
    fn from(item: &'a [u8]) -> Self {
        ToSQL::ByteSlice(item)
    }
}

impl<'a> From<&'a str> for ToSQL<'a> {
    fn from(item: &'a str) -> Self {
        ToSQL::CharSlice(item)
    }
}

impl<'a> From<&'a String> for ToSQL<'a> {
    fn from(item: &'a String) -> Self {
        ToSQL::CharSlice(item.as_str())
    }
}

// Translates Wallet Query Language to SQL
// WQL input is provided as a reference to a top level Operator
// Result is a tuple of query string and query arguments
pub(crate) fn wql_to_sql<'a>(
    class: &'a [u8],
    op: &'a Operator,
    _options: Option<&str>,
) -> Result<(String, Vec<ToSQL<'a>>), IndyError> {
    let mut arguments: Vec<ToSQL<'a>> = Vec::new();
    arguments.push(class.into());

    let clause_string = operator_to_sql(op, &mut arguments)?;

    const BASE: &str =
        "SELECT i.id, i.name, i.value, i.key, i.type FROM items as i WHERE i.type = ?";
    if !clause_string.is_empty() {
        let mut query_string = String::with_capacity(BASE.len() + 5 + clause_string.len());
        query_string.push_str(BASE);
        query_string.push_str(" AND ");
        query_string.push_str(&clause_string);
        Ok((query_string, arguments))
    } else {
        Ok((BASE.to_string(), arguments))
    }
}

pub(crate) fn wql_to_sql_count<'a>(
    class: &'a [u8],
    op: &'a Operator,
) -> Result<(String, Vec<ToSQL<'a>>), IndyError> {
    let mut arguments: Vec<ToSQL<'a>> = Vec::new();
    arguments.push(class.into());

    let clause_string = operator_to_sql(op, &mut arguments)?;
    let mut query_string = "SELECT count(*) FROM items as i WHERE i.type = ?".to_string();

    if !clause_string.is_empty() {
        query_string.push_str(" AND ");
        query_string.push_str(&clause_string);
    }

    Ok((query_string, arguments))
}

fn operator_to_sql<'a>(op: &'a Operator, arguments: &mut Vec<ToSQL<'a>>) -> IndyResult<String> {
    match *op {
        Operator::Eq(ref tag_name, ref target_value) => {
            eq_to_sql(tag_name, target_value, arguments)
        }
        Operator::Neq(ref tag_name, ref target_value) => {
            neq_to_sql(tag_name, target_value, arguments)
        }
        Operator::Gt(ref tag_name, ref target_value) => {
            gt_to_sql(tag_name, target_value, arguments)
        }
        Operator::Gte(ref tag_name, ref target_value) => {
            gte_to_sql(tag_name, target_value, arguments)
        }
        Operator::Lt(ref tag_name, ref target_value) => {
            lt_to_sql(tag_name, target_value, arguments)
        }
        Operator::Lte(ref tag_name, ref target_value) => {
            lte_to_sql(tag_name, target_value, arguments)
        }
        Operator::Like(ref tag_name, ref target_value) => {
            like_to_sql(tag_name, target_value, arguments)
        }
        Operator::In(ref tag_name, ref target_values) => {
            in_to_sql(tag_name, target_values, arguments)
        }
        Operator::And(ref suboperators) => and_to_sql(suboperators, arguments),
        Operator::Or(ref suboperators) => or_to_sql(suboperators, arguments),
        Operator::Not(ref suboperator) => not_to_sql(suboperator, arguments),
    }
}

fn eq_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value = ?))"
                    .to_string(),
            )
        }
        (
            TagName::EncryptedTagName(ref queried_name),
            TargetValue::Encrypted(ref queried_value),
        ) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_encrypted WHERE name = ? AND value = ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for equality operator",
        )),
    }
}

fn neq_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value != ?))"
                    .to_string(),
            )
        }
        (
            TagName::EncryptedTagName(ref queried_name),
            TargetValue::Encrypted(ref queried_value),
        ) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_encrypted WHERE name = ? AND value != ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for inequality operator",
        )),
    }
}

fn gt_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value > ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for $gt operator",
        )),
    }
}

fn gte_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value >= ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for $gte operator",
        )),
    }
}

fn lt_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value < ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for $lt operator",
        )),
    }
}

fn lte_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value <= ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for $lte operator",
        )),
    }
}

fn like_to_sql<'a>(
    name: &'a TagName,
    value: &'a TargetValue,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    match (name, value) {
        (TagName::PlainTagName(ref queried_name), TargetValue::Unencrypted(ref queried_value)) => {
            arguments.push(queried_name.into());
            arguments.push(queried_value.into());
            Ok(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value LIKE ?))"
                    .to_string(),
            )
        }
        _ => Err(err_msg(
            IndyErrorKind::WalletQueryError,
            "Invalid combination of tag name and value for $like operator",
        )),
    }
}

fn in_to_sql<'a>(
    name: &'a TagName,
    values: &'a Vec<TargetValue>,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    let mut in_string = String::new();
    match *name {
        TagName::PlainTagName(ref queried_name) => {
            in_string.push_str(
                "(i.id in (SELECT item_id FROM tags_plaintext WHERE name = ? AND value IN (",
            );
            arguments.push(queried_name.into());

            for (index, value) in values.iter().enumerate() {
                if let TargetValue::Unencrypted(ref target) = *value {
                    in_string.push('?');
                    arguments.push(target.into());
                    if index < values.len() - 1 {
                        in_string.push(',');
                    }
                } else {
                    return Err(err_msg(
                        IndyErrorKind::WalletQueryError,
                        "Encrypted tag value in $in for nonencrypted tag name",
                    ));
                }
            }

            Ok(in_string + ")))")
        }
        TagName::EncryptedTagName(ref queried_name) => {
            in_string.push_str(
                "(i.id in (SELECT item_id FROM tags_encrypted WHERE name = ? AND value IN (",
            );
            arguments.push(queried_name.into());
            let index_before_last = values.len() - 2;

            for (index, value) in values.iter().enumerate() {
                if let TargetValue::Encrypted(ref target) = *value {
                    in_string.push('?');
                    arguments.push(target.into());
                    if index <= index_before_last {
                        in_string.push(',');
                    }
                } else {
                    return Err(err_msg(
                        IndyErrorKind::WalletQueryError,
                        "Unencrypted tag value in $in for encrypted tag name",
                    ));
                }
            }

            Ok(in_string + ")))")
        }
    }
}

fn and_to_sql<'a>(
    suboperators: &'a [Operator],
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    join_operators(suboperators, " AND ", arguments)
}

fn or_to_sql<'a>(
    suboperators: &'a [Operator],
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    join_operators(suboperators, " OR ", arguments)
}

fn not_to_sql<'a>(suboperator: &'a Operator, arguments: &mut Vec<ToSQL<'a>>) -> IndyResult<String> {
    let suboperator_string = operator_to_sql(suboperator, arguments)?;
    Ok("NOT (".to_string() + &suboperator_string + ")")
}

fn join_operators<'a>(
    operators: &'a [Operator],
    join_str: &str,
    arguments: &mut Vec<ToSQL<'a>>,
) -> IndyResult<String> {
    let mut s = String::new();
    if !operators.is_empty() {
        s.push('(');
        for (index, operator) in operators.iter().enumerate() {
            let operator_string = operator_to_sql(operator, arguments)?;
            s.push_str(&operator_string);
            if index < operators.len() - 1 {
                s.push_str(join_str);
            }
        }
        s.push(')');
    }
    Ok(s)
}
