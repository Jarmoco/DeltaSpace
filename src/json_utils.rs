/* -----------------------------------------------------------------------------
 * JSON Value Builders
 *
 * Provides domain-specific constructors that convert internal data
 * structures (layers, diffs) into json::Value trees. Keeps the snapshot
 * and diff logic clean by separating concerns.
 * -------------------------------------------------------------------------- */

use std::collections::HashMap;

pub fn layers_to_json_value(layers: &HashMap<String, HashMap<String, u64>>) -> crate::json::Value {
    let mut outer = HashMap::new();
    for (depth, paths) in layers {
        let mut inner = HashMap::new();
        for (path, &size) in paths {
            inner.insert(path.clone(), crate::json::Value::Number(size as f64));
        }
        outer.insert(depth.clone(), crate::json::Value::Object(inner));
    }
    crate::json::Value::Object(outer)
}

pub fn diff_to_json_value(diff: &HashMap<String, (i64, u64)>) -> crate::json::Value {
    let mut obj = HashMap::new();
    for (path, &(d, current_size)) in diff {
        let mut entry = HashMap::new();
        entry.insert("diff".to_string(), crate::json::Value::Number(d as f64));
        entry.insert(
            "current".to_string(),
            crate::json::Value::Number(current_size as f64),
        );
        obj.insert(path.clone(), crate::json::Value::Object(entry));
    }
    crate::json::Value::Object(obj)
}
