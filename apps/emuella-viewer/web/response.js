// Keep Fetch-combined field values intact for the shared protocol parser.
export function beginResponse(client, tid, headers) {
  client.begin(tid, Array.from(headers.entries()));
}
