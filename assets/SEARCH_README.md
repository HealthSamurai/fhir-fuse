# Search

Create folder in resource type directory with the search query as the folder name. Search query is the query string after the ? in the URL.

```
./mnt/                                       # Mount point
├── ResourceType
│   ├── ?search/
│   │   └── search=query/
│   │       ├── Some-resource-type/
│   │       │   ├── Some-resource-type-id-1.json  # Resource version 1
│   │       │   └── Some-resource-type-id-2.json  # Resource version 2
│   │       └── Some-resource-type/
│   │           ├── Some-resource-type-id-1.json  # Resource version 1
│   │           └── Some-resource-type-id-2.json  # Resource version 2
│   ├── ResourceType-id-1.json
│   └── .ResourceType-id-1                    # Hidden dot folder with resource history
│       ├── ResourceType-version-id-1.json    # Resource version 1
│       └── ResourceType-version-id-2.json    # Resource version 2
├── Observation
|   ├── ?search/
|   |   └── _include=Observation:patient&_include:iterate=Patient:link/
│   │       ├── Observation/
│   │       │   ├── observation-id-1.json         # Resource version 1
│   │       │   └── observation-id-2.json         # Resource version 2
│   │       └── Patient/
│   │           ├── patient-id-1.json             # Resource version 1
│   │           └── patient-id-2.json             # Resource version 2
│   ├── observation-id-1.json
│   └── observation-id-2.json
```
