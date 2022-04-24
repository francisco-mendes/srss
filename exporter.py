import datetime
import sys
from pathlib import Path
from typing import Union

import openpyxl

sheet_name: str = open('sheet_name.secret.txt', encoding='utf-8').readline()


def export_to_excel(report_dir: str, excel_dir: str):
    """
    Reads the report files and writes each of them into the excel sheets.
    Assumes that the reports are from this month.

    :param report_dir: The directory containing the report files in raw (*.txt) format
    :param excel_dir: The directory containing the excel spreadsheets to be written to
    """
    for report_path in Path(report_dir).glob('*.txt'):
        print("report name:", report_path)
        records = read_report(report_path)
        excel_path = Path(excel_dir) / report_path.stem
        excel_path = excel_path.with_suffix('.xlsx')
        print("excel name:", excel_path)
        write_report(excel_path, records)


def read_report(report_path: Path) -> list[str]:
    """
    Reads the report file and returns its list of records.
    :param report_path: The path of the report file.
    :return: The records within
    """
    with open(report_path) as report_file:
        return [line.rstrip() for line in report_file]


def write_report(excel_path: Path, records: list[str]):
    """
    Writes the records to the excel spreadsheet.
    Assumes that the records are from the present month.
    :param excel_path: The path of the excel file.
    :param records: The records to write.
    """
    wb = openpyxl.load_workbook(str(excel_path))
    sheet = wb[sheet_name]

    column = offset_month(datetime.datetime.now())
    for index, record in enumerate(records):
        row = offset_day(index)
        sheet.cell(row=row, column=column, value=to_float(record))

    wb.save(excel_path.with_name(excel_path.stem + 'uwu.xlsx'))


def to_float(value: str) -> Union[float | None]:
    """
    Parses a float from a str or returns None if parsing fails.
    :param value: The string to parse
    :return: the corresponding float or None
    """
    try:
        return float(value)
    except ValueError:
        return None


def offset_day(index: int) -> int:
    """
    Offset for the row of the spreadsheet.
    :param index: The index of the record.
    :return: The row of the spreadsheet.
    """
    return index + 13


def offset_month(date: datetime.date) -> int:
    """
    Offset for the column of the spreadsheet, based on the date's month and year.
    :param date: The date to use as an index.
    :return: The column of the spreadsheet.
    """
    year = (date.year - 2021) * 12
    months = (year + date.month - 1) * 3
    return months + 2


if __name__ == '__main__':
    export_to_excel(sys.argv[1], sys.argv[2])
