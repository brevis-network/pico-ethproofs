import argparse
import json
import os
import sys
import urllib.request
import urllib.error

import time

# The error message to look for
ERROR_MESSAGE = "subblock-executor: failed to execute aggregation for validation"

def send_slack_alert(webhook_url, message):
    """Sends a message to a Slack Webhook URL."""
    payload = {
        "attachments": [
            {
                "color": "danger",
                "text": message,
                "mrkdwn_in": ["text"]
            }
        ]
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        webhook_url, 
        data=data, 
        headers={"Content-Type": "application/json"}
    )

    try:
        with urllib.request.urlopen(req) as response:
            if response.getcode() == 200:
                print("Slack alert sent successfully.")
            else:
                print(f"Failed to send Slack alert. Status code: {response.getcode()}")
    except urllib.error.URLError as e:
        print(f"Failed to send Slack alert. Error: {e}")

def monitor_log_file(log_file_path, webhook_url):
    """Continuously monitors the log file for the error message."""
    print(f"Monitoring {log_file_path} for errors...", flush=True)
    try:
        with open(log_file_path, 'r') as f:
            # Seek to the end of the file to start monitoring new log entries
            f.seek(0, os.SEEK_END)
            
            while True:
                line = f.readline()
                if not line:
                    time.sleep(0.1)  # Sleep briefly if no new line
                    continue
                
                if ERROR_MESSAGE in line:
                    print(f"Error found in {log_file_path}:", flush=True)
                    print(line.strip(), flush=True)
                    
                    alert_message = (
                        f"🚨 Error detected in log file `{log_file_path}`:\n"
                        f"```{line.strip()}```"
                    )
                    send_slack_alert(webhook_url, alert_message)
                    # Continue monitoring after alert
                    
    except FileNotFoundError:
        print(f"Error: Log file not found at {log_file_path}")
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nStopping log monitor.")
        sys.exit(0)
    except Exception as e:
        print(f"An error occurred while reading the log file: {e}")
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="Monitor log file for specific error messages.")
    parser.add_argument("log_file", help="Path to the log file to monitor")
    parser.add_argument("--webhook-url", help="Slack Webhook URL (optional, can also be set via SLACK_WEBHOOK_URL env var)")

    args = parser.parse_args()
    
    webhook_url = args.webhook_url

    # Try to load from .env.slack if not provided via args
    if not webhook_url:
        env_file = ".env.slack"
        if os.path.exists(env_file):
            try:
                with open(env_file, "r") as f:
                    for line in f:
                        line = line.strip()
                        if line.startswith("SLACK_WEBHOOK_URL="):
                            webhook_url = line.split("=", 1)[1].strip().strip('"').strip("'")
                            break
            except Exception as e:
                print(f"Warning: Failed to read {env_file}: {e}")

    webhook_url = webhook_url or os.environ.get("SLACK_WEBHOOK_URL")

    if not webhook_url:
        print("Error: Slack Webhook URL must be provided via --webhook-url argument or SLACK_WEBHOOK_URL environment variable.")
        sys.exit(1)

    monitor_log_file(args.log_file, webhook_url)

if __name__ == "__main__":
    main()
