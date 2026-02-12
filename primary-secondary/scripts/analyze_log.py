#!/usr/bin/env python3
"""
Aggregator Log Analysis Tool

Analyzes aggregator log files to extract performance metrics for each block
and outputs results in CSV format.

Usage:
    python analyze_log.py <log_file> [-o output.csv] [--tag TAG]

Output CSV columns:
    tag, kind, block, idx, status, cycles, e2e_s, log_file
"""

import argparse
import csv
import logging
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Set


@dataclass
class BlockResult:
    """Represents the analysis result for a single block execution."""

    block_number: int
    execution_id: int  # Unique ID for each execution of the same block
    status: str  # "OK" or "FAIL"
    cycles: Optional[int] = None
    e2e_s: Optional[float] = None
    error_message: Optional[str] = None
    start_timestamp: Optional[str] = None  # When block execution started
    end_timestamp: Optional[str] = None  # When block execution ended
    has_cycles: bool = False  # Whether cycles metric was found
    has_e2e: bool = False  # Whether E2E metric was found


class AggregatorLogAnalyzer:
    """Efficient analyzer for aggregator log files."""

    def __init__(self):
        # Regex patterns for parsing log entries
        # Detect the earliest indication of a new block on emulator lines
        # Example matches:
        #   [emulator] block-23290000: chunk_num=1
        #   [emulator-0] ... block-23290000 ...
        self.block_start_pattern = re.compile(r"\[emulator(?:-\d+)?\].*?block-(\d+)\b")
        self.total_cycles_pattern = re.compile(r"Total Cycles:\s*(\d+)")
        self.e2e_total_pattern = re.compile(r"E2E_total\s*=\s*([\d.]+)\s*s")
        self.timestamp_pattern = re.compile(
            r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2}))"
        )

        # Error patterns for failure detection
        self.error_patterns = [
            re.compile(r"ERROR", re.IGNORECASE),
            re.compile(r"FATAL", re.IGNORECASE),
            re.compile(r"panic", re.IGNORECASE),
            re.compile(r"failed", re.IGNORECASE),
            re.compile(r"timeout", re.IGNORECASE),
            re.compile(r"connection.*refused", re.IGNORECASE),
            re.compile(r"network.*error", re.IGNORECASE),
            re.compile(r"out of memory", re.IGNORECASE),
            re.compile(r"segmentation fault", re.IGNORECASE),
            re.compile(r"abort", re.IGNORECASE),
            re.compile(r"killed", re.IGNORECASE),
            re.compile(r"terminated", re.IGNORECASE),
        ]

        # Warning patterns that might indicate issues
        self.warning_patterns = [
            re.compile(r"WARN", re.IGNORECASE),
            re.compile(r"warning", re.IGNORECASE),
            re.compile(r"retry", re.IGNORECASE),
            re.compile(r"timeout", re.IGNORECASE),
        ]

        # Track current block being processed
        self.current_block: Optional[int] = None
        self.current_execution_id: int = 0
        self.block_results: List[BlockResult] = []
        self.pending_cycles: Optional[int] = None
        self.pending_e2e: Optional[float] = None

        # Track seen block numbers to detect duplicates or gaps
        self.seen_blocks: Set[int] = set()
        self.duplicate_blocks: Set[int] = set()

        # Statistics for validation
        self.total_lines_processed: int = 0
        self.malformed_lines: int = 0
        self.warning_count: int = 0

    def analyze_log_file(self, log_file_path: str) -> List[BlockResult]:
        """
        Analyze a log file and extract block results.

        Args:
            log_file_path: Path to the aggregator log file

        Returns:
            List of BlockResult objects for each block found
        """
        self.current_block = None
        self.current_execution_id = 0
        self.block_results = []

        logger = logging.getLogger(__name__)
        logger.info(f"Analyzing log file: {log_file_path}")

        try:
            with open(log_file_path, "r", encoding="utf-8", errors="replace") as f:
                line_count = 0
                for line in f:
                    line_count += 1
                    if line_count % 10000 == 0:
                        logger.info(f"Processed {line_count:,} lines...")

                    self._process_line(line.strip())

                logger.info(f"Completed processing {line_count:,} lines")

        except FileNotFoundError:
            logger.error(f"Error: Log file not found: {log_file_path}")
            sys.exit(1)
        except Exception as e:
            logger.error(f"Error reading log file: {e}")
            sys.exit(1)

        # Finalize any incomplete blocks
        self._finalize_current_block()

        return self.block_results

    def _process_line(self, line: str) -> None:
        """Process a single log line and update current block state."""
        self.total_lines_processed += 1

        # Extract timestamp if present
        timestamp_match = self.timestamp_pattern.search(line)
        timestamp = timestamp_match.group(1) if timestamp_match else None

        # Check for block start
        block_match = self.block_start_pattern.search(line)
        if block_match:
            # Finalize previous block if exists
            self._finalize_current_block()

            # Start new block execution
            try:
                block_num = int(block_match.group(1))
            except (ValueError, IndexError):
                self.malformed_lines += 1
                logging.getLogger(__name__).warning(
                    f"Malformed block number in line: {line[:100]}..."
                )
                return

            # Track duplicate blocks
            if block_num in self.seen_blocks:
                self.duplicate_blocks.add(block_num)
            else:
                self.seen_blocks.add(block_num)

            self.current_block = block_num
            self.current_execution_id += 1

            new_result = BlockResult(
                block_number=block_num,
                execution_id=self.current_execution_id,
                status="OK",  # Default to OK, will be updated if errors found
                start_timestamp=timestamp,
            )

            # If we have pending metrics, assign them to this new block
            if self.pending_cycles is not None:
                new_result.cycles = self.pending_cycles
                new_result.has_cycles = True
                self.pending_cycles = None

            if self.pending_e2e is not None:
                new_result.e2e_s = self.pending_e2e
                new_result.has_e2e = True
                self.pending_e2e = None

            self.block_results.append(new_result)
            return

        # Check for Total Cycles (can appear before block start)
        cycles_match = self.total_cycles_pattern.search(line)
        if cycles_match:
            try:
                cycles_value = int(cycles_match.group(1))
            except (ValueError, IndexError):
                self.malformed_lines += 1
                logging.getLogger(__name__).warning(
                    f"Malformed cycles value in line: {line[:100]}..."
                )
                return

            if self.current_block is not None and self.block_results:
                current_result = self.block_results[-1]
                if not current_result.has_cycles:
                    current_result.cycles = cycles_value
                    current_result.has_cycles = True
                else:
                    self.pending_cycles = cycles_value
            else:
                self.pending_cycles = cycles_value

        # Check for E2E total time (can appear before block start)
        e2e_match = self.e2e_total_pattern.search(line)
        if e2e_match:
            try:
                e2e_value = float(e2e_match.group(1))
            except (ValueError, IndexError):
                self.malformed_lines += 1
                logging.getLogger(__name__).warning(
                    f"Malformed E2E value in line: {line[:100]}..."
                )
                return

            if self.current_block is not None and self.block_results:
                current_result = self.block_results[-1]
                if not current_result.has_e2e:
                    current_result.e2e_s = e2e_value
                    current_result.has_e2e = True
                    current_result.end_timestamp = timestamp
                else:
                    self.pending_e2e = e2e_value
            else:
                self.pending_e2e = e2e_value

        # If we're in a block, check for errors and warnings
        if self.current_block is not None and self.block_results:
            current_result = self.block_results[-1]

            # Check for errors
            if any(pattern.search(line) for pattern in self.error_patterns):
                current_result.status = "FAIL"
                if current_result.error_message is None:
                    current_result.error_message = line[:200]

            # Check for warnings
            if any(pattern.search(line) for pattern in self.warning_patterns):
                self.warning_count += 1
                if current_result.error_message is None:
                    current_result.error_message = f"Warning detected: {line[:150]}"

    def _finalize_current_block(self) -> None:
        """Finalize the current block being processed."""
        if self.current_block is not None and self.block_results:
            result = self.block_results[-1]

            if result.status == "OK":
                missing_metrics = []

                if not result.has_cycles:
                    missing_metrics.append("cycles")
                if not result.has_e2e:
                    missing_metrics.append("e2e_s")

                if result.cycles is not None and result.cycles <= 0:
                    result.status = "FAIL"
                    result.error_message = f"Invalid cycles value: {result.cycles}"
                elif result.e2e_s is not None and result.e2e_s <= 0:
                    result.status = "FAIL"
                    result.error_message = f"Invalid E2E time value: {result.e2e_s}"
                elif missing_metrics:
                    result.status = "FAIL"
                    result.error_message = (
                        f"Missing required metrics: {', '.join(missing_metrics)}"
                    )

                if result.cycles is not None and result.cycles > 1_000_000_000:
                    if result.error_message is None:
                        result.error_message = (
                            f"Unusually high cycles: {result.cycles:,}"
                        )

                if result.e2e_s is not None and result.e2e_s > 3600:
                    if result.error_message is None:
                        result.error_message = (
                            f"Unusually long E2E time: {result.e2e_s:.1f}s"
                        )

            self.current_block = None

    def write_csv_output(
        self,
        results: List[BlockResult],
        output_path: str,
        log_file_path: str,
        tag: str = "",
    ) -> None:
        """Write results to CSV file."""
        output_file = Path(output_path)
        output_file.parent.mkdir(parents=True, exist_ok=True)

        sorted_results: List[BlockResult] = sorted(
            results, key=lambda x: (x.block_number, x.execution_id)
        )

        # Precompute count of executions per block to avoid O(n^2)
        per_block_counts: Dict[int, int] = {}
        for r in sorted_results:
            per_block_counts[r.block_number] = (
                per_block_counts.get(r.block_number, 0) + 1
            )

        logging.getLogger(__name__).info(
            f"Writing {len(sorted_results)} results to: {output_path}"
        )

        with open(output_path, "w", newline="", encoding="utf-8") as csvfile:
            writer = csv.writer(csvfile)

            writer.writerow(
                ["tag", "kind", "block", "idx", "status", "cycles", "e2e_s", "log_file"]
            )

            for result in sorted_results:
                block_identifier = str(result.block_number)
                if per_block_counts.get(result.block_number, 0) > 1:
                    block_identifier = f"{result.block_number}-{result.execution_id}"

                writer.writerow(
                    [
                        tag,
                        "aggregator",
                        block_identifier,
                        "",
                        result.status,
                        result.cycles if result.cycles is not None else "NA",
                        (
                            f"{result.e2e_s:.3f}" if result.e2e_s is not None else "NA"
                        ),
                        log_file_path,
                    ]
                )

        logging.getLogger(__name__).info(
            f"Successfully wrote CSV with {len(sorted_results)} entries"
        )

    def print_summary(self, results: List[BlockResult]) -> None:
        """Print a summary of the analysis results."""
        total_blocks = len(results)
        successful_blocks = sum(1 for r in results if r.status == "OK")
        failed_blocks = total_blocks - successful_blocks

        logger = logging.getLogger(__name__)
        logger.info(f"\n=== Analysis Summary ===")
        logger.info(f"Total blocks found: {total_blocks}")
        logger.info(f"Successful blocks: {successful_blocks}")
        logger.info(f"Failed blocks: {failed_blocks}")
        logger.info(f"Total lines processed: {self.total_lines_processed:,}")
        logger.info(f"Malformed lines: {self.malformed_lines}")
        logger.info(f"Warnings detected: {self.warning_count}")

        if self.duplicate_blocks:
            logger.info(f"Duplicate block executions: {sorted(self.duplicate_blocks)}")

        # Check for block number gaps
        if len(self.seen_blocks) > 1:
            min_block = min(self.seen_blocks)
            max_block = max(self.seen_blocks)
            expected_blocks = set(range(min_block, max_block + 1))
            missing_blocks = expected_blocks - self.seen_blocks
            if missing_blocks:
                logger.info(f"Missing block numbers: {sorted(missing_blocks)}")

        if successful_blocks > 0:
            cycles_values = [r.cycles for r in results if r.cycles is not None]
            e2e_values = [r.e2e_s for r in results if r.e2e_s is not None]

            if cycles_values:
                logger.info(
                    f"Cycles range: {min(cycles_values):,} - {max(cycles_values):,}"
                )
                logger.info(
                    f"Average cycles: {sum(cycles_values) / len(cycles_values):,.0f}"
                )
            if e2e_values:
                logger.info(
                    f"E2E time range: {min(e2e_values):.3f}s - {max(e2e_values):.3f}s"
                )
                logger.info(
                    f"Average E2E time: {sum(e2e_values) / len(e2e_values):.3f}s"
                )

        if failed_blocks > 0:
            logger.info(f"\nFailed blocks (showing first 10):")
            failed_count = 0
            for result in results:
                if result.status == "FAIL" and failed_count < 10:
                    logger.info(
                        f"  Block {result.block_number}-{result.execution_id}: {result.error_message}"
                    )
                    failed_count += 1
            if failed_blocks > 10:
                logger.info(f"  ... and {failed_blocks - 10} more failed blocks")


def _configure_logging() -> None:
    """Configure logging to stdout."""
    logger = logging.getLogger(__name__)
    if logger.handlers:
        return

    logger.setLevel(logging.INFO)

    stdout_handler = logging.StreamHandler(stream=sys.stdout)
    stdout_handler.setLevel(logging.INFO)

    formatter = logging.Formatter("%(message)s")
    stdout_handler.setFormatter(formatter)

    logger.addHandler(stdout_handler)


def main() -> None:
    """Main entry point for the script."""
    _configure_logging()

    parser = argparse.ArgumentParser(
        description="Analyze aggregator log files and extract block performance metrics",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    python analyze_log.py logs/pico-primary-20250901-120000.log
    python analyze_log.py logs/pico-primary-20250901-120000.log -o results.csv
    python analyze_log.py logs/pico-primary-20250901-120000.log results.csv --tag run1
        """,
    )

    parser.add_argument("log_file", help="Path to the aggregator log file")
    parser.add_argument("output_csv", nargs="?", help="Output CSV file path (optional)")
    parser.add_argument("--output", "-o", help="Output CSV file path (optional)")
    parser.add_argument("--tag", default="", help="Tag to include in CSV output")

    args = parser.parse_args()

    # Validate input file
    log_path = Path(args.log_file)
    if not log_path.exists():
        logging.getLogger(__name__).error(f"Error: Log file not found: {args.log_file}")
        sys.exit(1)

    if not log_path.is_file():
        logging.getLogger(__name__).error(f"Error: Path is not a file: {args.log_file}")
        sys.exit(1)

    if log_path.stat().st_size == 0:
        logging.getLogger(__name__).error(f"Error: Log file is empty: {args.log_file}")
        sys.exit(1)

    # Determine output path
    if args.output:
        output_path = Path(args.output)
    elif args.output_csv:
        output_path = Path(args.output_csv)
    else:
        output_path = log_path.parent / f"{log_path.stem}_analysis.csv"

    # Create analyzer and process log
    analyzer = AggregatorLogAnalyzer()
    results = analyzer.analyze_log_file(args.log_file)

    if not results:
        logging.getLogger(__name__).info("No blocks found in the log file.")
        sys.exit(1)

    # Write CSV output
    analyzer.write_csv_output(results, str(output_path), args.log_file, args.tag)

    # Print summary
    analyzer.print_summary(results)

    logging.getLogger(__name__).info(
        f"\nAnalysis complete. Results saved to: {output_path}"
    )


if __name__ == "__main__":
    main()
