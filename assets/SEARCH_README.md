# Search

Create folder in resource type directory with the search query as the folder name. Search query is the query string after the ? in the URL.

```
./mnt/                                       # Mount point
├── ResourceType
│   ├── _search/
│   │   └── search=query/
│   │       ├── Some-resource-type/
│   │       │   ├── Some-resource-type-id-1.json
│   │       │   └── Some-resource-type-id-2.json
│   │       └── Some-resource-type/
│   │           ├── Some-resource-type-id-1.json
│   │           └── Some-resource-type-id-2.json
│   ├── ResourceType-id-1.json
│   └── .ResourceType-id-1/                   # Hidden dot folder with resource history
│       ├── ResourceType-id-1.v1.json         # Resource version 1
│       └── ResourceType-id-1.v2.json         # Resource version 2
├── Observation
|   ├── _search/
|   |   └── _include=Observation:patient&_include:iterate=Patient:link/
│   │       ├── Observation/
│   │       │   ├── observation-id-1.json
│   │       │   └── observation-id-2.json
│   │       └── Patient/
│   │           ├── patient-id-1.json
│   │           └── patient-id-2.json
│   ├── observation-id-1.json
│   └── observation-id-2.json
```
