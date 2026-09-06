use compiler_core::{project::Project, run, runtime::RuntimeLimits};
use std::{fs, path::PathBuf};

#[test]
fn documented_examples_match_their_outputs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/language/examples");
    let mut count = 0;
    for file in fs::read_dir(&root).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|s| s != "itd") {
            continue;
        }
        let expected = path.with_extension("expected.txt");
        if !expected.exists() {
            continue;
        }
        count += 1;
        let source = fs::read_to_string(&path).unwrap();
        let result = run(&source);
        assert!(
            result.diagnostics.is_empty(),
            "{}: {:?}",
            path.display(),
            result.diagnostics
        );
        assert_eq!(
            result.output,
            fs::read_to_string(expected).unwrap(),
            "{}",
            path.display()
        );
        #[cfg(feature = "reference-interpreter")]
        {
            let analysis = compiler_core::analyze(&source);
            let reference = compiler_core::runtime::execute_reference(
                &analysis.program,
                analysis.semantic.as_ref().unwrap(),
            );
            assert_eq!(result.output, reference.output, "{}", path.display());
            assert_eq!(
                result.diagnostics,
                reference.diagnostics,
                "{}",
                path.display()
            );
        }
    }
    assert_eq!(count, 10);
    let project = Project::open(root.join("invoice")).unwrap();
    let result = project.run("main", RuntimeLimits::default(), None);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.output,
        fs::read_to_string(root.join("invoice/main.expected.txt")).unwrap()
    );
}
