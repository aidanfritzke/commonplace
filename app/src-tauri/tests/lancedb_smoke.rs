// Runtime smoke test for the LanceDB store path used by src/index.rs.
// Uses fake 3-dim vectors (no Ollama needed) to prove the exact API chain
// works at runtime: connect -> create_empty_table -> add -> filtered
// vector search (only_if + nearest_to + cosine + select) -> read results.

use std::sync::Arc;

use arrow_array::types::Float32Type;
use arrow_array::{Array, FixedSizeListArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::DistanceType;

#[tokio::test]
async fn lancedb_roundtrip_and_vault_filter() {
    let dir = std::env::temp_dir().join(format!("commonplace_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let conn = lancedb::connect(dir.to_str().unwrap())
        .execute()
        .await
        .unwrap();

    let dim = 3i32;
    let schema = Arc::new(Schema::new(vec![
        Field::new("vault", DataType::Utf8, false),
        Field::new("path", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
            false,
        ),
    ]));
    let table = conn
        .create_empty_table("chunks", schema.clone())
        .execute()
        .await
        .unwrap();

    let vault = StringArray::from(vec!["A", "A", "B"]);
    let path = StringArray::from(vec!["a1", "a2", "b1"]);
    let text = StringArray::from(vec!["apple", "banana", "cherry"]);
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        vec![
            Some(vec![Some(1.0), Some(0.0), Some(0.0)]), // a1 — closest to query
            Some(vec![Some(0.0), Some(1.0), Some(0.0)]), // a2
            Some(vec![Some(1.0), Some(0.0), Some(0.0)]), // b1 — close, but vault B
        ],
        dim,
    );
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(vault),
            Arc::new(path),
            Arc::new(text),
            Arc::new(vectors),
        ],
    )
    .unwrap();
    table.add(batch).execute().await.unwrap();

    // Search vault A only, nearest to [1,0,0]: a1 must be first, b1 must not appear.
    let stream = table
        .query()
        .only_if("vault = 'A'")
        .nearest_to(vec![1.0f32, 0.0, 0.0])
        .unwrap()
        .distance_type(DistanceType::Cosine)
        .select(Select::Columns(vec!["path".into(), "text".into()]))
        .limit(5)
        .execute()
        .await
        .unwrap();
    let batches: Vec<RecordBatch> = stream.try_collect().await.unwrap();

    let mut paths = Vec::new();
    for b in &batches {
        let i = b.schema().index_of("path").unwrap();
        let col = b.column(i).as_any().downcast_ref::<StringArray>().unwrap();
        for r in 0..b.num_rows() {
            paths.push(col.value(r).to_string());
        }
    }

    assert!(!paths.is_empty(), "expected results, got none");
    assert_eq!(paths[0], "a1", "nearest result should be a1, got {:?}", paths);
    assert!(
        !paths.contains(&"b1".to_string()),
        "vault filter leaked vault B: {:?}",
        paths
    );

    // Delete by predicate (mirrors incremental re-index), then confirm it's gone.
    table.delete("vault = 'A' AND path = 'a1'").await.unwrap();
    let stream2 = table
        .query()
        .only_if("vault = 'A'")
        .nearest_to(vec![1.0f32, 0.0, 0.0])
        .unwrap()
        .limit(5)
        .execute()
        .await
        .unwrap();
    let after: Vec<RecordBatch> = stream2.try_collect().await.unwrap();
    let mut paths2 = Vec::new();
    for b in &after {
        let i = b.schema().index_of("path").unwrap();
        let col = b.column(i).as_any().downcast_ref::<StringArray>().unwrap();
        for r in 0..b.num_rows() {
            paths2.push(col.value(r).to_string());
        }
    }
    assert!(
        !paths2.contains(&"a1".to_string()),
        "a1 should be deleted, still present: {:?}",
        paths2
    );

    let _ = std::fs::remove_dir_all(&dir);
}
