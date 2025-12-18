# Fisle system design

## Basic structure

Per each resource type, there is a directory with the resource type name.

```
./mnt/                              # Mount point
├── ResourceType/                   # Directory for resource type
│   ├── ResourceType-id-1.json      # Each file is a FHIR resource
│   └── ResourceType-id-2.json      # Filename is the resource id
├── Patient/
│   ├── patient-id-1.json
│   └── patient-id-2.json
├── ...
```

## CRUD

Create: Create a new resource file in the resource type directory.
Read: Read a resource file in the resource type directory.
Update: Update a resource file in the resource type directory.
Delete: Delete a resource file in the resource type directory.

## History

Resource history is stored in a hidden dot folder:

```
./mnt/                                       # Mount point
├── ResourceType/
│   ├── ResourceType-id-1.json
│   └── .ResourceType-id-1/                  # Hidden dot folder with resource history
│       ├── ResourceType-version-id-1.json   # Resource version 1
│       └── ResourceType-version-id-2.json   # Resource version 2
├── Patient/
│   ├── patient-id-1.json
│   └── patient-id-2.json
├── Observation/
│   ├── observation-id-1.json
│   └── observation-id-2.json
```

## Search

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


## FHIR Operations

```
./mnt/                                       # Mount point
├── ResourceType
│   └── $opearation/
│       ├── operation=arguments/
│       │   └── result.(json|csv)  # Resource version 1
│       └── operation=arguments/
│           └── result.(json|csv)  # Resource version 2
└── ViewDefinition
    ├── $run/
    │   ├── viewReference=blood_pressure/
    │   │   └── result.csv                    # Resource version 2
    │   └── viewReference=patient_demographics/
    │       └── result.csv                    # Resource version 2
    │── patient_demographics.json      # Each file is a FHIR resource
    └── blood_pressure.json      # Each file is a FHIR resource
```
