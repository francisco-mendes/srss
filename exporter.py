from datetime import date as Date
import sys
from pathlib import Path
from typing import Union
import xlwings as xl

sheet_name: str = open('sheet_name.secret.txt', encoding='utf-8').readline()


def export_to_excel(report_dir: str, excel_dir: str):
    """
    Reads the report files and writes each of them into the excel sheets.
    Assumes that the reports are from this month.
    """
    with xl.App(add_book=False) as app:
        for report_path in Path(report_dir).glob('*.txt'):
            station = report_path.stem
            print('processing', station)
            excel_path = Path(excel_dir) / station / f'Registos_de_Producao_PV_{station}.xlsx'

            records = read_report(report_path)
            write_report(app, excel_path, records)


def read_report(report_path: Path) -> list[str]:
    """
    Reads the report file and returns its list of records.
    """
    with open(report_path) as report_file:
        return [line.rstrip() for line in report_file]


def write_report(app: xl.App, excel_path: Path, records: list[str], date: Date = Date.today()):
    """
    Writes the records to the excel spreadsheet, according to the given date, defaulting to today.

    """
    options = {'update_links': True, 'ignore_read_only_recommended': True, 'editable': True}
    wb: xl.Book = app.books.open(excel_path, **options)
    sheet: xl.Sheet = wb.sheets[sheet_name]
    sheet.activate()

    column = offset_month(date)
    start, end = offset_day(records)
    cells: xl.Range = sheet.range((start, column), (end, column))
    cells.value = [[to_float(v)] for v in records]

    wb.save(excel_path.with_name(excel_path.stem + 'uwu.xlsx'))


def to_float(value: str) -> Union[float | None]:
    """
    Parses a float from a str or returns None if parsing fails.
    """
    try:
        return float(value)
    except ValueError:
        return None


def offset_day(records: list) -> tuple[int, int]:
    """
    Creates offsets for insertion into the worksheet for the given list of records.
    """
    return 13, len(records) + 12


def offset_month(date: Date) -> int:
    """
    Offset for the column of the spreadsheet, based on the date's month and year.
    """
    year = (date.year - 2021) * 12
    months = (year + date.month - 1) * 3
    return months + 2


if __name__ == '__main__':
    export_to_excel(sys.argv[1], sys.argv[2])
