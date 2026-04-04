use std::collections::BTreeMap;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostingServiceError {
    UnsupportedGitService,
    RemoteUrlParseFailed,
}

#[cfg(test)]
impl HostingServiceError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::UnsupportedGitService => "Unsupported git service",
            Self::RemoteUrlParseFailed => "Failed to parse repo information from url",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ServiceDefinition {
    provider: &'static str,
    pull_request_url_into_default_branch: &'static str,
    pull_request_url_into_target_branch: &'static str,
    commit_url: &'static str,
    regex_strings: &'static [&'static str],
    repo_url_template: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct ServiceDomain {
    git_domain: &'static str,
    web_domain: &'static str,
    service_definition: ServiceDefinition,
}

#[derive(Debug, Clone)]
struct CustomServiceDomain {
    git_domain: String,
    web_domain: String,
    service_definition: ServiceDefinition,
}

#[derive(Debug, Clone)]
enum CandidateServiceDomain {
    Default(ServiceDomain),
    Custom(CustomServiceDomain),
}

impl CandidateServiceDomain {
    fn git_domain(&self) -> &str {
        match self {
            Self::Default(service_domain) => service_domain.git_domain,
            Self::Custom(service_domain) => &service_domain.git_domain,
        }
    }

    fn web_domain(&self) -> &str {
        match self {
            Self::Default(service_domain) => service_domain.web_domain,
            Self::Custom(service_domain) => &service_domain.web_domain,
        }
    }

    fn service_definition(&self) -> ServiceDefinition {
        match self {
            Self::Default(service_domain) => service_domain.service_definition,
            Self::Custom(service_domain) => service_domain.service_definition,
        }
    }
}

#[derive(Debug, Clone)]
struct Service {
    repo_url: String,
    definition: ServiceDefinition,
}

const DEFAULT_URL_REGEX_STRINGS: &[&str] = &[
    r"^(?:https?|ssh)://[^/]+/(?P<owner>.*)/(?P<repo>.*?)(?:\.git)?$",
    r"^(.*?@)?.*:/*(?P<owner>.*)/(?P<repo>.*?)(?:\.git)?$",
];

const DEFAULT_REPO_URL_TEMPLATE: &str = "https://{{.webDomain}}/{{.owner}}/{{.repo}}";

const GITHUB_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "github",
    pull_request_url_into_default_branch: "/compare/{{.From}}?expand=1",
    pull_request_url_into_target_branch: "/compare/{{.To}}...{{.From}}?expand=1",
    commit_url: "/commit/{{.CommitHash}}",
    regex_strings: DEFAULT_URL_REGEX_STRINGS,
    repo_url_template: DEFAULT_REPO_URL_TEMPLATE,
};

const BITBUCKET_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "bitbucket",
    pull_request_url_into_default_branch: "/pull-requests/new?source={{.From}}&t=1",
    pull_request_url_into_target_branch: "/pull-requests/new?source={{.From}}&dest={{.To}}&t=1",
    commit_url: "/commits/{{.CommitHash}}",
    regex_strings: &[
        r"^(?:https?|ssh)://.*/(?P<owner>.*)/(?P<repo>.*?)(?:\.git)?$",
        r"^.*@.*:/*(?P<owner>.*)/(?P<repo>.*?)(?:\.git)?$",
    ],
    repo_url_template: DEFAULT_REPO_URL_TEMPLATE,
};

const GITLAB_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "gitlab",
    pull_request_url_into_default_branch:
        "/-/merge_requests/new?merge_request%5Bsource_branch%5D={{.From}}",
    pull_request_url_into_target_branch:
        "/-/merge_requests/new?merge_request%5Bsource_branch%5D={{.From}}&merge_request%5Btarget_branch%5D={{.To}}",
    commit_url: "/-/commit/{{.CommitHash}}",
    regex_strings: DEFAULT_URL_REGEX_STRINGS,
    repo_url_template: DEFAULT_REPO_URL_TEMPLATE,
};

const AZDO_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "azuredevops",
    pull_request_url_into_default_branch: "/pullrequestcreate?sourceRef={{.From}}",
    pull_request_url_into_target_branch: "/pullrequestcreate?sourceRef={{.From}}&targetRef={{.To}}",
    commit_url: "/commit/{{.CommitHash}}",
    regex_strings: &[
        r"^.+@vs-ssh\.visualstudio\.com[:/](?:v3/)?(?P<org>[^/]+)/(?P<project>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?$",
        r"^git@ssh.dev.azure.com.*/(?P<org>.*)/(?P<project>.*)/(?P<repo>.*?)(?:\.git)?$",
        r"^https://.*@dev.azure.com/(?P<org>.*?)/(?P<project>.*?)/_git/(?P<repo>.*?)(?:\.git)?$",
        r"^https://.*/(?P<org>.*?)/(?P<project>.*?)/_git/(?P<repo>.*?)(?:\.git)?$",
    ],
    repo_url_template: "https://{{.webDomain}}/{{.org}}/{{.project}}/_git/{{.repo}}",
};

const BITBUCKET_SERVER_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "bitbucketServer",
    pull_request_url_into_default_branch: "/pull-requests?create&sourceBranch={{.From}}",
    pull_request_url_into_target_branch:
        "/pull-requests?create&targetBranch={{.To}}&sourceBranch={{.From}}",
    commit_url: "/commits/{{.CommitHash}}",
    regex_strings: &[
        r"^ssh://git@.*/(?P<project>.*)/(?P<repo>.*?)(?:\.git)?$",
        r"^https://.*/scm/(?P<project>.*)/(?P<repo>.*?)(?:\.git)?$",
    ],
    repo_url_template: "https://{{.webDomain}}/projects/{{.project}}/repos/{{.repo}}",
};

const GITEA_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "gitea",
    pull_request_url_into_default_branch: "/compare/{{.From}}",
    pull_request_url_into_target_branch: "/compare/{{.To}}...{{.From}}",
    commit_url: "/commit/{{.CommitHash}}",
    regex_strings: DEFAULT_URL_REGEX_STRINGS,
    repo_url_template: DEFAULT_REPO_URL_TEMPLATE,
};

const CODEBERG_SERVICE_DEF: ServiceDefinition = ServiceDefinition {
    provider: "codeberg",
    pull_request_url_into_default_branch: "/compare/{{.From}}",
    pull_request_url_into_target_branch: "/compare/{{.To}}...{{.From}}",
    commit_url: "/commit/{{.CommitHash}}",
    regex_strings: DEFAULT_URL_REGEX_STRINGS,
    repo_url_template: DEFAULT_REPO_URL_TEMPLATE,
};

const SERVICE_DEFINITIONS: &[ServiceDefinition] = &[
    GITHUB_SERVICE_DEF,
    BITBUCKET_SERVICE_DEF,
    GITLAB_SERVICE_DEF,
    AZDO_SERVICE_DEF,
    BITBUCKET_SERVER_SERVICE_DEF,
    GITEA_SERVICE_DEF,
    CODEBERG_SERVICE_DEF,
];

const DEFAULT_SERVICE_DOMAINS: &[ServiceDomain] = &[
    ServiceDomain {
        service_definition: GITHUB_SERVICE_DEF,
        git_domain: "github.com",
        web_domain: "github.com",
    },
    ServiceDomain {
        service_definition: BITBUCKET_SERVICE_DEF,
        git_domain: "bitbucket.org",
        web_domain: "bitbucket.org",
    },
    ServiceDomain {
        service_definition: GITLAB_SERVICE_DEF,
        git_domain: "gitlab.com",
        web_domain: "gitlab.com",
    },
    ServiceDomain {
        service_definition: AZDO_SERVICE_DEF,
        git_domain: "dev.azure.com",
        web_domain: "dev.azure.com",
    },
    ServiceDomain {
        service_definition: GITEA_SERVICE_DEF,
        git_domain: "try.gitea.io",
        web_domain: "try.gitea.io",
    },
    ServiceDomain {
        service_definition: CODEBERG_SERVICE_DEF,
        git_domain: "codeberg.org",
        web_domain: "codeberg.org",
    },
];

pub(crate) fn pull_request_url_for_remote(
    remote_url: &str,
    from: &str,
    to: Option<&str>,
    config_service_domains: &BTreeMap<String, String>,
    logged_errors: &mut Vec<String>,
) -> Result<String, HostingServiceError> {
    HostingServiceMgr::new(remote_url, config_service_domains).get_pull_request_url(
        from,
        to.unwrap_or_default(),
        logged_errors,
    )
}

pub(crate) fn commit_browser_url_for_remote(
    remote_url: &str,
    commit_oid: &str,
    config_service_domains: &BTreeMap<String, String>,
    logged_errors: &mut Vec<String>,
) -> Result<String, HostingServiceError> {
    HostingServiceMgr::new(remote_url, config_service_domains)
        .get_commit_url(commit_oid, logged_errors)
}

struct HostingServiceMgr<'a> {
    remote_url: &'a str,
    config_service_domains: &'a BTreeMap<String, String>,
}

impl<'a> HostingServiceMgr<'a> {
    fn new(remote_url: &'a str, config_service_domains: &'a BTreeMap<String, String>) -> Self {
        Self {
            remote_url,
            config_service_domains,
        }
    }

    fn get_pull_request_url(
        &self,
        from: &str,
        to: &str,
        logged_errors: &mut Vec<String>,
    ) -> Result<String, HostingServiceError> {
        let service = self.get_service(logged_errors)?;
        let from = url_encode_component(from);
        if to.is_empty() {
            return Ok(service.resolve_url(
                service.definition.pull_request_url_into_default_branch,
                &[("From", from.as_str())],
            ));
        }

        let to = url_encode_component(to);
        Ok(service.resolve_url(
            service.definition.pull_request_url_into_target_branch,
            &[("From", from.as_str()), ("To", to.as_str())],
        ))
    }

    fn get_commit_url(
        &self,
        commit_hash: &str,
        logged_errors: &mut Vec<String>,
    ) -> Result<String, HostingServiceError> {
        let service = self.get_service(logged_errors)?;
        Ok(service.resolve_url(
            service.definition.commit_url,
            &[("CommitHash", commit_hash)],
        ))
    }

    fn get_service(&self, logged_errors: &mut Vec<String>) -> Result<Service, HostingServiceError> {
        let service_domain = self.get_service_domain(logged_errors)?;
        let definition = service_domain.service_definition();
        let repo_url = definition
            .get_repo_url_from_remote_url(self.remote_url, service_domain.web_domain())?;
        Ok(Service {
            repo_url,
            definition,
        })
    }

    fn get_service_domain(
        &self,
        logged_errors: &mut Vec<String>,
    ) -> Result<CandidateServiceDomain, HostingServiceError> {
        for service_domain in self.get_candidate_service_domains(logged_errors) {
            if self.remote_url.contains(service_domain.git_domain()) {
                return Ok(service_domain);
            }
        }
        Err(HostingServiceError::UnsupportedGitService)
    }

    fn get_candidate_service_domains(
        &self,
        logged_errors: &mut Vec<String>,
    ) -> Vec<CandidateServiceDomain> {
        let mut service_domains = DEFAULT_SERVICE_DOMAINS
            .iter()
            .copied()
            .map(CandidateServiceDomain::Default)
            .collect::<Vec<_>>();

        for (git_domain, type_and_domain) in self.config_service_domains {
            let Some((provider, web_domain)) = type_and_domain.split_once(':') else {
                logged_errors.push(format!(
                    "Unexpected format for git service: '{}'. Expected something like 'github.com:github.com'",
                    type_and_domain
                ));
                continue;
            };

            if web_domain.matches(':').count() > 1 {
                logged_errors.push(format!(
                    "Unexpected format for git service: '{}'. Expected something like 'github.com:github.com'",
                    type_and_domain
                ));
                continue;
            }

            let Some(service_definition) = SERVICE_DEFINITIONS
                .iter()
                .copied()
                .find(|definition| definition.provider == provider)
            else {
                let provider_names = SERVICE_DEFINITIONS
                    .iter()
                    .map(|definition| definition.provider)
                    .collect::<Vec<_>>()
                    .join(", ");
                logged_errors.push(format!(
                    "Unknown git service type: '{}'. Expected one of {}",
                    provider, provider_names
                ));
                continue;
            };

            service_domains.push(CandidateServiceDomain::Custom(CustomServiceDomain {
                git_domain: git_domain.clone(),
                web_domain: web_domain.to_string(),
                service_definition,
            }));
        }

        service_domains
    }
}

impl ServiceDefinition {
    fn get_repo_url_from_remote_url(
        &self,
        remote_url: &str,
        web_domain: &str,
    ) -> Result<String, HostingServiceError> {
        for regex_str in self.regex_strings {
            let regex = Regex::new(regex_str).expect("service regexes are valid");
            let Some(mut args) = find_named_matches(&regex, remote_url) else {
                continue;
            };

            args.insert("webDomain".to_string(), web_domain.to_string());
            return Ok(resolve_template(self.repo_url_template, &args));
        }

        Err(HostingServiceError::RemoteUrlParseFailed)
    }
}

fn find_named_matches(regex: &Regex, input: &str) -> Option<BTreeMap<String, String>> {
    let captures = regex.captures(input)?;
    let mut results = BTreeMap::new();

    for (index, value) in captures.iter().enumerate().skip(1) {
        let key = regex
            .capture_names()
            .nth(index)
            .flatten()
            .unwrap_or("")
            .to_string();
        results.insert(
            key,
            value.map_or_else(String::new, |capture| capture.as_str().to_string()),
        );
    }

    Some(results)
}

impl Service {
    fn resolve_url(&self, template: &str, args: &[(&str, &str)]) -> String {
        let mut values = BTreeMap::new();
        for (key, value) in args {
            values.insert((*key).to_string(), (*value).to_string());
        }
        format!("{}{}", self.repo_url, resolve_template(template, &values))
    }
}

fn resolve_template(template: &str, args: &BTreeMap<String, String>) -> String {
    let mut resolved = template.to_string();
    for (key, value) in args {
        resolved = resolved.replace(&format!("{{{{.{key}}}}}"), value);
    }
    resolved
}

fn url_encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        commit_browser_url_for_remote, find_named_matches, pull_request_url_for_remote,
        HostingServiceError,
    };
    use regex::Regex;
    use std::collections::BTreeMap;

    struct PullRequestScenario<'a> {
        test_name: &'a str,
        from: &'a str,
        to: &'a str,
        remote_url: &'a str,
        config_service_domains: BTreeMap<String, String>,
        expected_url: Option<&'a str>,
        expected_error: Option<&'a str>,
        expected_logged_errors: &'a [&'a str],
    }

    #[test]
    fn pull_request_urls_match_lazygit_hosting_service_behavior() {
        let scenarios = vec![
            PullRequestScenario {
                test_name: "bitbucket default branch",
                from: "feature/profile-page",
                to: "",
                remote_url: "git@bitbucket.org:johndoe/social_network.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://bitbucket.org/johndoe/social_network/pull-requests/new?source=feature%2Fprofile-page&t=1"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "github specific target branch",
                from: "feature/sum-operation",
                to: "feature/operations",
                remote_url: "git@github.com:peter/calculator.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://github.com/peter/calculator/compare/feature%2Foperations...feature%2Fsum-operation?expand=1"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "gitlab nested groups",
                from: "feature/ui",
                to: "",
                remote_url: "git@gitlab.com:peter/public/calculator.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://gitlab.com/peter/public/calculator/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fui"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "azure devops server custom mapping",
                from: "feature/new",
                to: "",
                remote_url: "https://mycompany.azuredevops.com/collection/myproject/_git/myrepo",
                config_service_domains: BTreeMap::from([(
                    "mycompany.azuredevops.com".to_string(),
                    "azuredevops:mycompany.azuredevops.com".to_string(),
                )]),
                expected_url: Some("https://mycompany.azuredevops.com/collection/myproject/_git/myrepo/pullrequestcreate?sourceRef=feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "bitbucket server target branch",
                from: "feature/new",
                to: "dev",
                remote_url: "https://mycompany.bitbucket.com/scm/myproject/myrepo.git",
                config_service_domains: BTreeMap::from([(
                    "mycompany.bitbucket.com".to_string(),
                    "bitbucketServer:mycompany.bitbucket.com".to_string(),
                )]),
                expected_url: Some("https://mycompany.bitbucket.com/projects/myproject/repos/myrepo/pull-requests?create&targetBranch=dev&sourceBranch=feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "gitea target branch",
                from: "feature/new",
                to: "dev",
                remote_url: "https://mycompany.gitea.io/myproject/myrepo.git",
                config_service_domains: BTreeMap::from([(
                    "mycompany.gitea.io".to_string(),
                    "gitea:mycompany.gitea.io".to_string(),
                )]),
                expected_url: Some("https://mycompany.gitea.io/myproject/myrepo/compare/dev...feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "codeberg target branch",
                from: "feature/new",
                to: "dev",
                remote_url: "https://codeberg.org/johndoe/myrepo.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://codeberg.org/johndoe/myrepo/compare/dev...feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "unsupported service",
                from: "feature/divide-operation",
                to: "",
                remote_url: "git@something.com:peter/calculator.git",
                config_service_domains: BTreeMap::new(),
                expected_url: None,
                expected_error: Some("Unsupported git service"),
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "logs malformed config entry",
                from: "feature/profile-page",
                to: "",
                remote_url: "git@bitbucket.org:johndoe/social_network.git",
                config_service_domains: BTreeMap::from([(
                    "noservice.work.com".to_string(),
                    "noservice.work.com".to_string(),
                )]),
                expected_url: Some("https://bitbucket.org/johndoe/social_network/pull-requests/new?source=feature%2Fprofile-page&t=1"),
                expected_error: None,
                expected_logged_errors: &["Unexpected format for git service: 'noservice.work.com'. Expected something like 'github.com:github.com'"],
            },
            PullRequestScenario {
                test_name: "logs unknown provider",
                from: "feature/profile-page",
                to: "",
                remote_url: "git@bitbucket.org:johndoe/social_network.git",
                config_service_domains: BTreeMap::from([(
                    "invalid.work.com".to_string(),
                    "noservice:invalid.work.com".to_string(),
                )]),
                expected_url: Some("https://bitbucket.org/johndoe/social_network/pull-requests/new?source=feature%2Fprofile-page&t=1"),
                expected_error: None,
                expected_logged_errors: &["Unknown git service type: 'noservice'. Expected one of github, bitbucket, gitlab, azuredevops, bitbucketServer, gitea, codeberg"],
            },
            PullRequestScenario {
                test_name: "encodes reserved characters",
                from: "feature/someIssue#123",
                to: "master",
                remote_url: "git@gitlab.com:me/public/repo-with-issues.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://gitlab.com/me/public/repo-with-issues/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2FsomeIssue%23123&merge_request%5Btarget_branch%5D=master"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "github extra slash remote",
                from: "feature/sum-operation",
                to: "feature/operations",
                remote_url: "git@github.com:/peter/calculator.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://github.com/peter/calculator/compare/feature%2Foperations...feature%2Fsum-operation?expand=1"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "gitlab ssh alias remote with custom service domain",
                from: "feature/sum-operation",
                to: "feature/operations",
                remote_url: "gitlab:peter/calculator.git",
                config_service_domains: BTreeMap::from([(
                    "gitlab".to_string(),
                    "gitlab:gitlab.com".to_string(),
                )]),
                expected_url: Some("https://gitlab.com/peter/calculator/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fsum-operation&merge_request%5Btarget_branch%5D=feature%2Foperations"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "azure devops legacy ssh alias remote",
                from: "feature/new",
                to: "",
                remote_url: "git@vs-ssh.visualstudio.com:v3/myorg/myproject/myrepo",
                config_service_domains: BTreeMap::from([(
                    "vs-ssh.visualstudio.com".to_string(),
                    "azuredevops:dev.azure.com".to_string(),
                )]),
                expected_url: Some("https://dev.azure.com/myorg/myproject/_git/myrepo/pullrequestcreate?sourceRef=feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "custom service domain with port",
                from: "feature/new",
                to: "",
                remote_url: "https://my.domain.test/johndoe/social_network.git",
                config_service_domains: BTreeMap::from([(
                    "my.domain.test".to_string(),
                    "gitlab:my.domain.test:1111".to_string(),
                )]),
                expected_url: Some("https://my.domain.test:1111/johndoe/social_network/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fnew"),
                expected_error: None,
                expected_logged_errors: &[],
            },
            PullRequestScenario {
                test_name: "encodes reserved characters in target branch",
                from: "feature/new",
                to: "archive/never-ending-feature#666",
                remote_url: "git@gitlab.com:me/public/repo-with-issues.git",
                config_service_domains: BTreeMap::new(),
                expected_url: Some("https://gitlab.com/me/public/repo-with-issues/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fnew&merge_request%5Btarget_branch%5D=archive%2Fnever-ending-feature%23666"),
                expected_error: None,
                expected_logged_errors: &[],
            },
        ];

        for scenario in scenarios {
            let mut logged_errors = Vec::new();
            let result = pull_request_url_for_remote(
                scenario.remote_url,
                scenario.from,
                (!scenario.to.is_empty()).then_some(scenario.to),
                &scenario.config_service_domains,
                &mut logged_errors,
            );

            assert_ne!(
                scenario.expected_url.is_some(),
                scenario.expected_error.is_some(),
                "invalid scenario configuration"
            );

            if let Some(expected_url) = scenario.expected_url {
                assert_eq!(result.unwrap(), expected_url, "{}", scenario.test_name);
            } else if let Some(expected_error) = scenario.expected_error {
                let error = result.unwrap_err();
                assert_eq!(error.message(), expected_error, "{}", scenario.test_name);
            }

            assert_eq!(
                logged_errors,
                scenario
                    .expected_logged_errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                "{}",
                scenario.test_name
            );
        }
    }

    #[test]
    fn commit_urls_match_expected_service_templates() {
        let scenarios = [
            (
                "github",
                "git@github.com:peter/calculator.git",
                BTreeMap::new(),
                "https://github.com/peter/calculator/commit/abcdef1234",
            ),
            (
                "gitlab",
                "git@gitlab.com:peter/calculator.git",
                BTreeMap::new(),
                "https://gitlab.com/peter/calculator/-/commit/abcdef1234",
            ),
            (
                "bitbucket",
                "git@bitbucket.org:johndoe/social_network.git",
                BTreeMap::new(),
                "https://bitbucket.org/johndoe/social_network/commits/abcdef1234",
            ),
            (
                "azuredevops",
                "git@ssh.dev.azure.com:v3/myorg/myproject/myrepo",
                BTreeMap::new(),
                "https://dev.azure.com/myorg/myproject/_git/myrepo/commit/abcdef1234",
            ),
            (
                "azuredevops legacy ssh alias",
                "git@vs-ssh.visualstudio.com:v3/myorg/myproject/myrepo",
                BTreeMap::from([(
                    "vs-ssh.visualstudio.com".to_string(),
                    "azuredevops:dev.azure.com".to_string(),
                )]),
                "https://dev.azure.com/myorg/myproject/_git/myrepo/commit/abcdef1234",
            ),
            (
                "bitbucketServer",
                "https://mycompany.bitbucket.com/scm/myproject/myrepo.git",
                BTreeMap::from([(
                    "mycompany.bitbucket.com".to_string(),
                    "bitbucketServer:mycompany.bitbucket.com".to_string(),
                )]),
                "https://mycompany.bitbucket.com/projects/myproject/repos/myrepo/commits/abcdef1234",
            ),
            (
                "gitea",
                "https://mycompany.gitea.io/myproject/myrepo.git",
                BTreeMap::from([(
                    "mycompany.gitea.io".to_string(),
                    "gitea:mycompany.gitea.io".to_string(),
                )]),
                "https://mycompany.gitea.io/myproject/myrepo/commit/abcdef1234",
            ),
            (
                "codeberg",
                "git@codeberg.org:johndoe/myrepo.git",
                BTreeMap::new(),
                "https://codeberg.org/johndoe/myrepo/commit/abcdef1234",
            ),
            (
                "github extra slash remote",
                "git@github.com:/peter/calculator.git",
                BTreeMap::new(),
                "https://github.com/peter/calculator/commit/abcdef1234",
            ),
        ];

        for (label, remote_url, config_service_domains, expected_url) in scenarios {
            let mut logged_errors = Vec::new();
            let url = commit_browser_url_for_remote(
                remote_url,
                "abcdef1234",
                &config_service_domains,
                &mut logged_errors,
            )
            .unwrap();
            assert_eq!(url, expected_url, "{label}");
            assert!(logged_errors.is_empty(), "{label}");
        }
    }

    #[test]
    fn unsupported_commit_service_returns_error() {
        let mut logged_errors = Vec::new();
        let error = commit_browser_url_for_remote(
            "git@something.com:peter/calculator.git",
            "abcdef1234",
            &BTreeMap::new(),
            &mut logged_errors,
        )
        .unwrap_err();
        assert_eq!(error, HostingServiceError::UnsupportedGitService);
        assert!(logged_errors.is_empty());
    }

    #[test]
    fn find_named_matches_matches_upstream_regexp_cases() {
        let scenarios = [
            (
                Regex::new(r"^(?P<name>\w+)").unwrap(),
                "hello world",
                Some(BTreeMap::from([(
                    String::from("name"),
                    String::from("hello"),
                )])),
            ),
            (
                Regex::new(r"^https?://.*/(?P<owner>.*)/(?P<repo>.*?)(\.git)?$").unwrap(),
                "https://my_username@bitbucket.org/johndoe/social_network.git",
                Some(BTreeMap::from([
                    (String::new(), String::from(".git")),
                    (String::from("owner"), String::from("johndoe")),
                    (String::from("repo"), String::from("social_network")),
                ])),
            ),
            (
                Regex::new(r"(?P<owner>hello) world").unwrap(),
                "yo world",
                None,
            ),
        ];

        for (regex, input, expected) in scenarios {
            assert_eq!(find_named_matches(&regex, input), expected, "{input}");
        }
    }
}
