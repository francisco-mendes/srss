from datetime import date as Date
import time
import random
import sys
import msvcrt
from pathlib import Path
from typing import Union, Iterator
import xlwings as xl
import re

sheet_name: str = open('sheet_name.secret.txt', encoding='utf-8').readline()


def export_to_excel(report_dir: str, excel_dirs: [str]):
    """
    Reads the report files and writes each of them into the excel sheets.
    Assumes that the reports are from this month.
    """
    app: xl.App
    with xl.App(add_book=False) as app:
        for station, excel_path in report_sheets(excel_dirs):
            try:
                report_path = Path(report_dir) / f'{station}.log'
                station = report_path.stem

                wait_or_timeout(random.randint(60, 90))
                print('processing', station)

                records = read_report(report_path)
                write_report(app, excel_path, list(records))
            except FileNotFoundError:
                print('no report found for station', station)


def wait_or_timeout(timeout: int = 5):
    start = time.time()
    while True:
        if msvcrt.kbhit():
            msvcrt.getch()
            break
        elif time.time() - start > timeout:
            break
        time.sleep(0.5)


def report_sheets(excel_dirs: [str]) -> Iterator[tuple[str, Path]]:
    """
    Finds and yields the name and path of every report sheet inside the given directories
    """
    pattern = re.compile('Registos de Produção PV (.+?).xlsx')
    for excel_dir in excel_dirs:
        for path in Path(excel_dir).rglob('*.xlsx'):
            match = pattern.search(str(path))
            if match:
                yield match.group(1), path


def read_report(report_path: Path) -> Iterator[tuple[Date, float]]:
    """
    Reads the report file and returns its list of records.
    """
    pattern = re.compile(r'\[(\d+?)-(\d+?)-(\d+?)]: (\S+)')
    with open(report_path) as report_file:
        for line in report_file:
            year, month, day, value = pattern.match(line).groups()
            yield Date(int(year), int(month), int(day)), to_float(value)


def write_report(app: xl.App, excel_path: Path, records: [tuple[Date, float]]):
    """
    Writes the records to the excel spreadsheet, according to the given date, defaulting to today.
    """
    options = {'update_links': True, 'ignore_read_only_recommended': True, 'editable': True}
    wb: xl.Book = app.books.open(excel_path, **options)
    sheet: xl.Sheet = wb.sheets[sheet_name]
    sheet.activate()

    for date, value in records:
        column = offset_month(date)
        row = offset_day(date)
        sheet[row, column].value = value

    wb.save(excel_path)
    wb.close()


def to_float(value: str) -> Union[float | None]:
    """
    Parses a float from a str or returns None if parsing fails.
    """
    try:
        return float(value)
    except ValueError:
        return 0.0


def offset_day(date: Date) -> int:
    """
    Creates offsets for insertion into the worksheet for the given list of records.
    """
    return 11 + date.day


def offset_month(date: Date) -> int:
    """
    Offset for the column of the spreadsheet, based on the date's month and year.
    """
    year = (date.year - 2021) * 12
    months = (year + date.month - 1) * 3
    return months + 1


if __name__ == '__main__':
    export_to_excel(sys.argv[1], sys.argv[2:])
