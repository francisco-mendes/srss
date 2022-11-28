# srss
Solar Report Scraping Software

A web scraper to export reports from a dashboard and write them to log files and a python script to open and edit excel files with the new files.

## Dependencies

Requires:
* [Cargo] to build the rust executable, 
* [Python] 3 with [xlwings] installed to run the excel edit script, 
* Chrome and a [Chrmedriver] executable with matching versions and
* \*.secret.txt files containig a single line with sensitive information:
  * sheet_name: the name of the sheet of the excel file to edit,
  * src\loginpage: the url to the login page,
  * src\reportpage: the url to the report page for each station, sans the station id,
  * src\stationlink: the regex to obtain the id of a station from a station link, containing a named capture group named `id`.

## Build
To install xlwings run this, possibly using pip from a virtual environment rather than a global one:
```sh
pip install xlwings pytimedinput
```

To build the project run:
```sh
cargo b --release
```
This will create a `srss` executable in `target\release`.

## Running the scraper
```sh
cargo b --release
# or using the executable:
.\srss
```

For a help message use:
```sh
cargo b --release -- -h
# or using the executable:
.\srss -h
```
The options and arguments required should be shown by the help message.

## Running the exporter
```
python exporter.py <report-dir> <excel-dir1> <excel-dir2>...
```
Example:
```sh
python exporter.py report/ out_dir/ 
```