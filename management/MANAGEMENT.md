This project is conducted under a formal management procedure. The top-level rules are:
1) Every (new) development must be backed by formal requirements. The requirements may fall into Business, Engineering, or Operations categories and should be placed into
   their own files: BUSINESS.md, ENGINEERING.md, and OPERATIONS.md. Those files will have the requirements numbered – so they may be referenced everywhere – and should
   always correspond to the desired behavior of the software.
2) When the current state of the software contains a different set of information than on those requirement files, a "sync" must be performed. Either by:
   - Reviewing the requirement;
   - Reviewing the code;
   - Adding a new requirement;
   - Removing the code.
   A `requirement_drift` detection operation -- described below -- may be performed by the AI agent, but updating the requirements is responsability of the human.
3) The requirements listed on the files will be used to indicate clearly the ticket number that tackles them, so they must be formally classified into sections – see below.
4) Other documentation: although the use of diagrams, summaries, and whatnot are important tools in communicating complex software behavior, the source of truth should be the ones
   described below. If a diagram exists on the git repo and is updated, corresponding changes must be immediately done to the appropriate source.
   The source of truth is:
   * Requirements are authoritative about desired behavior;
   * Unit and Integration Tests are evidence that selected behavior has been verified;
   * The code in `main` is evidence of actual behavior; code in other branches are evidence of "behavior under development";
   * A disagreement between them is a formally detectable state called "requirement drift", which may further classified as defects, undocumented accepted behavior, incomplete
     implementation, or obsolete requirements.
5) Work Items, including epics, stories, tasks, bugs, technical-debt remediation, experiments, and similar units of planned work, must be defined in the corresponding backlog
   files BUSINESS.backlog.md, ENGINEERING.backlog.md, and OPERATIONS.backlog.md. These files offer the opportunity to describe what should be done to transform the current
   state of the software into the state described by the requirement.
6) Version Control: the branch `main` contains production-grade code that is ready for automatic deployment to staging and is a candidate for production release.
   Production release requires all applicable post-merge QA and release checks to have succeeded. On top of these rules, no known technical
   debt classified as "release-blocking" may enter `main`. Non-blocking debt must be explicitly documented as a new entry in the "Engineering" or "Operational" requirements file,
   with justification and deadline for improvement. This is more important for design decisions, APIs, and models that may be used as foundation for future code.
7) Each Work Item must be associated with its formal branch, with the following rules:
   * Naming the branches should always be in the form of either: "<developer_username>/RM.HHH.rr.ss-ttt" for task, bugs, or story branches; "feature/RM.HHH.rr.ss-ttt" for feature branches, where:
     - 'R' stands for the (R)equirements document – either 'B', 'E', or 'O';
     - 'M' stands for the (M)otivation of the change – either a (N)ew requirement, a (R)evisit of an already implemented requirement which changed or which needed a different
       implementation approach, a (F)ix for a faulty or buggy implementation of a requirement, or even an e(X)perimental branch, a prototype, R&D, or spike which should never be
       merged to `main`;
     - 'HHH' is the abbreviated (H)eader section as described on the requirements document. Each header have their abbreviation (with a more or less, 3 letters) inside quotes, on their title;
     - A special name is "GEN" – for "general" requirements that do not fall into any deployment; Sometimes, requirements may be made for libs such as "HLC" (high level crates)
       or "LLC" (low level crates);
     - 'rr' is the requirement number within that header name;
     - 'ss' is the requirement sub-topic -- letters: a, b, c, ... z, aa, ab, ...;
     - 'ttt' is a zero-padded sequential number – counting Work Items to the same requirement subtopic – and avoids both branch and backlog collision.
8) CI/CD: Every commit to `main` will be rolled out to a `staging` environment -- except if it only touches documentation or files that don't change the deployables;
   if a git tag is created (semver version numbers), a rollout to production will be made. Additionally:
   * Any deployment windows must be manually honored by the one in charge of creating the git tags
   * git tags for release candidates -- e.g., "1.2.3-rc.1" will not be rolled out; they are meant for testing prior to a rollout.
   * If a git tag using the release candidate format is rejected, any changes should create yet a new release candidate tag, incrementing candidate number.
9) Each Work Item may have the following states on the respective backlog file: "Under Planning", "Planned", "Started", "In Code Review", "Integrated" (optional; applies only to child
   Work Items that have been merged into their parent feature branch.), "QA" (optional; depends on the story; not valid for work targeting a feature branch), "Merged", and "Rolled Out".
   The non-happy paths include "Rejected", "Cancelled" and "Superseded by <other work item id>"
   All those statuses must carry – and keep – the date the Work Item entered them. Also, "Blocked" may happen at any point. It is a property and not a new state.
10) Automation: AI may be used to create the following scripts to automate what was discussed here in the following terms:
    * Engineering work:
      - `start_work <BRANCH_NAME>`: Will parse the requirements and backlog files and will:
        - check if the '<BRANCH_NAME>' is appropriate (according to these rules) and correctly references a work in the "Started" state in the backlog, also checking if the
          backlog lists the current user as the one responsible for the work;
        - create and switch to the new branch from `origin/main` – if it is an ordinary task, bug, or story;
        - or make sure a feature branch exists (branch it from `origin/main` if it doesn't), the creating a new branch from the feature branch;
        - exposes the verbatim description of the work, as present in the appropriate backlog file;
        - ask if the user wants to engage in a chat in which the engineer may discuss what needs to be done.
      - `chat_about`: Uses the current branch name and does the same as the above-mentioned "ask if the user wants to engage in a chat in which the engineer may discuss what
        needs to be done";
      - `verification_check`: Verifies if the requirements in the current brach are covered:
        - Functional behavior: Unit/integration/system tests
        - Performance: Benchmarks or load tests
        - Security: Tests, analysis, threat review, audit
        - Operations> Staging evidence, monitoring, drills
        - Maintainability: Review and static analysis
        - Disaster recovery: Recovery exercise
        - UX or product behavior: Acceptance review or experiment
      - `review`: Uses the current branch name to review the code against the project good practices and the description of the story, checking for completeness.
    * Manager:
      - `evaluate_plan <BRANCH_NAME>`: Will parse the requirements and backlogs, telling if the proposed epic, story, bug report, or task makes sense in regard to the requirement.
      - `draft_plan <REQUIREMENT_ID>`: Similar to the above, but meant for the AI to create the entries in the backlog for the referred requirement, putting it in the state "Under Planning";
      - `advance_state <BRANCH_NAME>`: Do the following steps on the up-to-date `main` branch:
        - move the given backlog reference to the next state, using today (on the local timezone) as the date reference;
        - show the previous state alongside with the requirement and backlog Work Item descriptions;
        - ask for an "Engineer Name" when transitioning from "Planned" to "Started" – and place the name on the backlog;
      - `chase_techdebts`: Look at the code and see if there are opportunities for:
        - refactoring out common parts of the code which are somewhat duplicated – provided they relate to solving similar problems;
        - find potential performance or resource utilization issues;
        - find potential security issues;
        - lists other tech debts found, especially those at the foundations – in which new code will be added to and, thus, will grow worse with time;
    * Product Manager:
      - `estimate_requirement <REQUIREMENT_ID>`: Interprets what is written under that reference and tells:
        - if it is clear and articulated, or if it needs rephrasing for better understanding;
        - if it seems like a good fit for the current state of things, taking the market and competitors into account;
        - tells if this can be promptly implemented or if other requirements must be tackled first – either existing or not – helping creating a "dependency list", if any.
        - give effort estimations in senior developer / hour terms.
      - `audit_requirements`: A good pass over all requirements to infer:
        - as above, also tells if any requirement is not too clear and articulated, or if it needs rephrasing for better understanding;
        - are there any redundant requirements? which ones? and why?
        - conflicting requirements;
        - circular dependencies;
        - requirements that describe implementation rather than outcome;
        - requirements that combine unrelated obligations;
        - requirements with undefined actors or subjects;
        - requirements with undefined failure behavior;
        - requirements with unbounded terms such as “fast,” “reliable,” or “appropriate”;
        - requirements whose acceptance would contradict another deployment;
        - requirements with no work and no evidence of implementation;
        - implemented behavior with no governing requirement;
        - stale requirements referring to removed components;
        - requirements whose deadlines, versions, or market assumptions have expired.
        - could a set of requirements be made more clear or broader without adding too much development cost?
        - are there any requirements that appear to not fit well within the current system scope, market, or competitive environment? Why? If it is a scope-miss thing,
          which prior requirements are likely missing?
        - detect and report on any "requirement drift", as mentioned above;
      - `sync_requirement <REQUIREMENT_ID>`: Meant for requirements already rolled out: evaluate any discrepancies between what is expressed in the requirements and what is present in the code:
        - the code should be evaluated as a whole to inspect the state of things;
        - a detailed report should be given on possible differences in interpretation between what the code does and what it was meant to do;
        - the AI may infer the quality, telling if there are tech-debts associated with that item, if there are security issues, inefficient code, or even bugs;
        - the existing branch in the git repo might be used to give the AI additional context of the initial implementation;
        - the AI may indicate which branches changed the implementation / interpretation of that requirement as time passed and code changed; 