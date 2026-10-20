// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Topological sort for fix plan operations.

use super::types::{FixPlan, Operation};
use anyhow::{bail, Result};
use std::collections::{HashMap, VecDeque};

/// Sort operations respecting `depends_on` edges.
///
/// Independent operations keep plan order. Returns an error for duplicate ids,
/// unknown dependencies and dependency cycles instead of silently dropping
/// operations.
pub fn topological_sort(plan: &FixPlan) -> Result<Vec<&Operation>> {
    if plan.operations.is_empty() {
        return Ok(Vec::new());
    }

    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut op_map: HashMap<&str, &Operation> = HashMap::new();

    for op in &plan.operations {
        if op_map.insert(op.id.as_str(), op).is_some() {
            bail!("duplicate operation id in plan: {}", op.id);
        }
        in_degree.entry(op.id.as_str()).or_insert(0);
        adj.entry(op.id.as_str()).or_default();
    }

    for op in &plan.operations {
        for dep_id in &op.depends_on {
            if !op_map.contains_key(dep_id.as_str()) {
                bail!(
                    "operation {} depends on unknown operation {}",
                    op.id,
                    dep_id
                );
            }
            adj.entry(dep_id.as_str()).or_default().push(op.id.as_str());
            *in_degree.entry(op.id.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = plan
        .operations
        .iter()
        .map(|op| op.id.as_str())
        .filter(|id| in_degree.get(id) == Some(&0))
        .collect();

    let mut sorted = Vec::new();

    while let Some(node) = queue.pop_front() {
        if let Some(&op) = op_map.get(node) {
            sorted.push(op);
        }
        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    if sorted.len() != plan.operations.len() {
        bail!("dependency cycle detected among plan operations");
    }

    Ok(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::plan::types::*;

    fn op(id: &str, deps: &[&str]) -> Operation {
        Operation {
            id: id.to_string(),
            op_type: OperationType::FileEdit(FileEdit {
                file: "/etc/x".into(),
                backup: false,
                changes: vec![],
            }),
            priority: Priority::Low,
            description: String::new(),
            risk: Priority::Low,
            reversible: true,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            validation: None,
            undo: None,
        }
    }

    fn plan(ops: Vec<Operation>) -> FixPlan {
        FixPlan {
            version: "1".into(),
            vm: "d".into(),
            generated: chrono::Utc::now(),
            profile: "p".into(),
            overall_risk: "low".into(),
            estimated_duration: "1s".into(),
            metadata: PlanMetadata {
                author: "t".into(),
                review_required: false,
                reversible: true,
                description: None,
                tags: vec![],
            },
            operations: ops,
            post_apply: vec![],
        }
    }

    #[test]
    fn keeps_plan_order_and_respects_deps() {
        let p = plan(vec![
            op("a", &["c"]),
            op("b", &[]),
            op("c", &[]),
            op("d", &[]),
        ]);
        let ids: Vec<_> = topological_sort(&p)
            .unwrap()
            .iter()
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(ids, ["b", "c", "d", "a"]);
    }

    #[test]
    fn cycle_is_an_error() {
        let p = plan(vec![op("a", &["b"]), op("b", &["a"]), op("c", &[])]);
        assert!(topological_sort(&p).is_err());
    }

    #[test]
    fn unknown_dependency_is_an_error() {
        let p = plan(vec![op("a", &["missing"])]);
        assert!(topological_sort(&p).is_err());
    }
}
