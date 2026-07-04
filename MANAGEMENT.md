This project is conducted under a formal management procedure. The top-level rules are:
1) Every (new) development must be backed off by formal requisites. The requisites may fall into Business, Engineering, or Operations categories and should be placed into
   their own files: BUSINESS.md, ENGINEERING.md, and OPERATIONS.md. Those files will have the requisites numbered – so they may be referenced everywhere – and should
   always correspond to the desired behavior of the software.
2) When the current state of the software contains more information than on those files, a corresponding entry must be added to the requirements files.
3) The requirements listed on the files must indicate clearly the ticket number that tackles them – see bellow.
4) Other documentations: although the use of diagrams, summaries, and whatnot are important tool in communicating complex software behavior, the source of truth should be both the
   requirement documents and software. If a diagram is updated, corresponding changes must be immediately done to the appropriate source.
5) Actionable Work -- that is, epics, tasks, bugs, stories, etc. -- should be defined in the corresponding files BUSINESS.backlog.md, ENGINEERING.backlog.md, and
   OPERATIONS.backlog.md. Those files offer the opportunity to describe what should be done to transform the current state of the software into the state described by the
   requirement. 
6) Version Control: the branch `main` contains code that is ready to be deployed. Only production-grade code should reach that branch. On top of that rule, no code that
   brings in bad APIs, bad Interfaces, bad modeling, should ever make it to `main` even if the code is correct and robust from an implementation point of view: from an
   engineering point of view, such code adds debts to every future new code that is built on top of it. In other words, "known tech-debth code should never make it to `main`".
7) Each Actionable Work should reside in their formal branch, with the following rules:
   * Naming the branches should always be in the form of either: targeted to `main`: "RM.DDD.rr.ss", or targeted to a feature branch: "RM.DDD.rr.ss-t", where:
     - 'R' stands for the (R)equirements document – either 'B', 'E', or 'O';
     - 'M' stands for the (M)otivation of the change – either a (N)ew requirement, a (R)evisit of an already implemented requirement which changed or which needed a different
       implementation approach, a (F)ix for a faulty or buggy implementation of a requirement, or even a (-), standing for a prototype, R&D, or spike which should never be merged
       to `main`;
     - 'DDD' is the abbreviated (D)eployment name as described on the requirements document -- for now we only have "BOT";
     - A special name is "GEN" -- for "general" requirements that do not fall into any deployment; Sometimes, requirements may be made for libs such as "HLC" (high level crates)
       or "LLC" (low level crates);
     - `rr` is the requirement number within that deployment name;
     - `ss` is the requirement sub-topic -- letters: a, b, c, ... z, aa, ab, ...;
     - 't' is meant only to be used if that requirement subtopic is being tackled by an epic with multiple tasks, bugs or stories. In this case, 't' should be a sequential number
       and each of these branches should never be allowed to be merged to `main` -- they should be first merged to their feature branch first; only then, the whole feature branch
       will eventually make it to `main` -- after tests, etc. The feature branch, of course, will be in the form "RM.DDD.rr.ss"
     - `t` is simply a sequential number in the backlog files aiming at progressing the software in the desired direction -- a requisite sub-topic is often devided into several
8) CI/CD: Every commit to `main` will be rolled out to a `staging` environment; if a git tag is created (semver version numbers), a rollout to production will be made. Additionally:
   * Any deployment windows must be manually honored by the one in charge of creating the git tags
   * git tags prefixed by '_' will not be rolled out; they are meant for testing prior to a rollout.
   * If a git tag using the "_semver" format is rejected, any changes should create yet a new tag, incrementing the patch version of the semver number.
9) Each Actionable Work may have the following states on the respective backlog file: "Under Planning", "Planned", "Started", "In Code Review", "QA" (optional; depends on the story),
   "Merged", and "Rolled Out". All those statuses must carry – and keep – the date the actionable work entered them.
10) Automation: AI may be used to create the following scripts to automate what was discussed here in the following terms:
    * Engineering work:
      - `start_work <BRANCH_NAME>`: Will parse the requirements and backlog files and will:
        - check if the '<BRANCH_NAME>' is appropriate (according to these rules) and correctly references a work not far from the "Planned" state in the backlog;
        - create and switch to the new branch from the current remote `main`;
        - exposes the verbatim description of the work, as present in the appropriate backlog file;
        - ask if the user wants to engage in a chat in which the engineer may discuss what needs to be done.
      - `chat_about`: Uses the current branch name and does the same as the above mentioned "ask if the user wants to engage in a chat in which the engineer may discuss what
        needs to be done"
      - `review`: Uses the current branch name to review the code against the project good practices and the description of the story, checking for completeness.
    * Manager:
      - `evaluate_plan <BRANCH_NAME>`: Will parse the requirements and backlogs, telling if the proposed epic, story, bug report, or task makes sense in regards to the requirement.
      - `draft_plan <BRANCH_NAME>`: Similar to the above, but meant for the AI to create the entries in the backlog for the referred requirement, putting it in the state "Under Planning";
      - `advance_state <BRANCH_NAME>`: Do the following steps on the up-to-date `main` branch:
        - move the given backlog reference to the next state, using today as the date reference;
        - show the previous state alongside with the requirement and backlog actionable work descriptions;
        - ask for an "Engineer Name" when transitioning from "Planned" to "Started" – and place the name on the backlong;
      - `chase_techdebts`: Look at the code and see if there are opportunities for:
        - refactoring out common parts of the code which are somewhat duplicated – provided they relate to solving similar problems;
        - find potential performance or resource utilization issues;
        - find potential security issues;
        - enumerates other tech-debts found, specially those at the foundations -- in which new code will be added to and, thus, will grow worse with time;
    * Product Manager:
      - `estimate_requirement <BRANCH_NAME>`: Interprets what is written under that reference and tells:
        - if it is clear and articulated, or if it needs rephrasing for better understanding;
        - if it seems like a good fit for the current state of things, taking the market and competitors into account;
        - tells if this can be promptly implemented or if other requirements must be tackled first -- either existing or not -- helping creating a "dependency list", if any.
        - give effort estimations in senior developer / hour terms.
      - `optimize_requirements`: A good pass over all requirements to infer:
        - as above, also tells if any requirement is not too clear and articulated, or if it need rephrasing for better understanding;
        - are there any redundant requirements? which ones? and why?
        - could a set of requirements be made more clear or broader without adding too much development cost?
        - are there any requirements that appear to not fit well within the current system scope, market, or competitive environment? Why? If it is a scope-miss thing,
          which prior requirements are likely missing?
      - `sync_requirement <BRANCH_NAME>`: Meant for requirements already rolled out: evaluate any discrepancies between what is expressed in the requirements and what is present in the code:
        - the code should be evaluated as a whole to inspect the state of things;
        - a detailed report should be given on possible differences in interpretation between what the code does and what it was meant to do;
        - the AI may infer the quality, telling if there are tech-debts associated with that item, if there are security issues, inneficient code, or even bugs;
        - the existing branch in the git repo might be used to give the AI additional context of the initial implementation;
        - the AI may indicate which branches changed the implementation / interpretation of that requirement as time passed and code changed; 