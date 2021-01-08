<a name="unreleased"></a>
## [Unreleased]


<a name="0.3.0"></a>
## [0.3.0] - 2021-01-08
### Feat
- **cli:** add summary for tags
- **lib:** implement the cost_of_life calculation


<a name="0.2.4"></a>
## [0.2.4] - 2021-01-03
### Chore
- **cli:** minor tweaks to the cli interface

### Doc
- better documentation

### Fix
- regexp for tags


<a name="0.2.3"></a>
## [0.2.3] - 2021-01-02
### Doc
- update readme

### Tests
- add wasm tests


<a name="0.2.2"></a>
## [0.2.2] - 2021-01-02
### Chore
- **lib:** improve error management and test coverage

### Feat
- **lib:** tx tags are returned in alphabetical order

### Fix
- **lib:** end_date should be the last inclusive date
- **lib:** magic number for 100% should be 1.0
- **lib:** date calculation off of one day

### Test
- **cli:** add tests for datastore


<a name="0.2.1"></a>
## [0.2.1] - 2020-12-28
### Fix
- **cli:** honour the "--on" parameter
- **cli:** print the correct app name and version

### Test
- **lib:** add more tests for the lib part


<a name="0.2.0"></a>
## [0.2.0] - 2020-12-27
### Chore
- fix clippy warnings

### Feat
- add summary command
- **wasm:** simplify return types


<a name="0.1.2"></a>
## [0.1.2] - 2020-12-07
### Feat
- print the cost of life from the cli

### Fix
- bumping version to fix a wasm-pack issue


<a name="0.1.1"></a>
## [0.1.1] - 2020-12-07
### Build
- add makefile

### Chore
- refactor code and add wasm support

### Feat
- persist data
- add basic command line
- add support for months
- **lang:** accept . (dot) as a tag prefix
- **lib:** add progess calculation


<a name="0.1.0"></a>
## 0.1.0 - 2020-11-30

[Unreleased]: https://github.com/noandrea/costoflife-rs/compare/0.3.0...HEAD
[0.3.0]: https://github.com/noandrea/costoflife-rs/compare/0.2.4...0.3.0
[0.2.4]: https://github.com/noandrea/costoflife-rs/compare/0.2.3...0.2.4
[0.2.3]: https://github.com/noandrea/costoflife-rs/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/noandrea/costoflife-rs/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/noandrea/costoflife-rs/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/noandrea/costoflife-rs/compare/0.1.2...0.2.0
[0.1.2]: https://github.com/noandrea/costoflife-rs/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/noandrea/costoflife-rs/compare/0.1.0...0.1.1
