use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Distribution {
    pub description: String,
    pub tools: HashMap<String, f64>,
}

pub type DistributionMap = HashMap<String, Distribution>;

pub fn load_distributions(path: &Path) -> Result<DistributionMap, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read distributions file: {e}"))?;
    let map: DistributionMap =
        serde_yaml::from_str(&content).map_err(|e| format!("Invalid distributions YAML: {e}"))?;
    Ok(map)
}

pub fn sample_tools(distribution: &Distribution) -> Vec<String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut selected: Vec<String> = distribution
        .tools
        .iter()
        .filter(|(_, &prob)| rng.gen_range(0.0..100.0) < prob)
        .map(|(name, _)| name.clone())
        .collect();

    if selected.is_empty() {
        if let Some((best_name, _)) = distribution
            .tools
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            selected.push(best_name.clone());
        }
    }

    selected.sort();
    selected
}

pub fn list_distribution_names(distributions: &DistributionMap) -> Vec<(String, String)> {
    let mut names: Vec<_> = distributions
        .iter()
        .map(|(name, dist)| (name.clone(), dist.description.clone()))
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_distributions() {
        let path = Path::new("training/distributions.yaml");
        let dists = load_distributions(path).expect("Should load distributions");
        assert!(dists.contains_key("default"));
        assert!(dists.contains_key("research"));
        assert!(dists.contains_key("development"));
        let default = &dists["default"];
        assert_eq!(*default.tools.get("bash").unwrap(), 100.0);
    }

    #[test]
    fn test_sample_always_returns_at_least_one() {
        let dist = Distribution {
            description: "test".into(),
            tools: HashMap::from([("bash".into(), 0.001)]),
        };
        for _ in 0..100 {
            let selected = sample_tools(&dist);
            assert!(!selected.is_empty());
        }
    }

    #[test]
    fn test_sample_default_returns_all() {
        let dist = Distribution {
            description: "test".into(),
            tools: HashMap::from([
                ("bash".into(), 100.0),
                ("web_search".into(), 100.0),
            ]),
        };
        let selected = sample_tools(&dist);
        assert!(selected.contains(&"bash".to_string()));
        assert!(selected.contains(&"web_search".to_string()));
    }

    #[test]
    fn test_list_distribution_names() {
        let mut dists = DistributionMap::new();
        dists.insert("beta".into(), Distribution {
            description: "B desc".into(),
            tools: HashMap::new(),
        });
        dists.insert("alpha".into(), Distribution {
            description: "A desc".into(),
            tools: HashMap::new(),
        });
        let names = list_distribution_names(&dists);
        assert_eq!(names[0].0, "alpha");
        assert_eq!(names[1].0, "beta");
    }
}
