internal static class OutputFormat
{
    public static string FormatAddress(ulong address) => $"0x{address:x12}";

    public static string JoinUuids(IReadOnlyList<Guid> uuids) =>
        uuids.Count == 0 ? "" : string.Join(",", uuids.Select(uuid => uuid.ToString()));
}
