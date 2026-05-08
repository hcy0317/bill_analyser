use bill_analyser_core::governance_manifest_snapshot;

fn main() {
    let snapshot = governance_manifest_snapshot();
    let payload =
        serde_json::to_string_pretty(&snapshot).expect("governance manifest should serialize");
    println!("{payload}");
}
