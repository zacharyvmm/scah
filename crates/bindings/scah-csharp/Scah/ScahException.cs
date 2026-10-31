namespace Scah;

/// <summary>Why a call into scah failed.</summary>
/// <remarks>Mirrors <c>ScahStatus</c> in <c>scah.h</c>.</remarks>
public enum ScahStatus
{
    /// <summary>A required argument was missing.</summary>
    NullPointer = 1,

    /// <summary>An input string was not valid UTF-8.</summary>
    InvalidUtf8 = 2,

    /// <summary>A selector could not be compiled.</summary>
    InvalidSelector = 3,

    /// <summary>A parse was attempted without any queries.</summary>
    EmptyQueries = 4,

    /// <summary>The HTML nests elements deeper than the parser supports.</summary>
    MaximumDepthExceeded = 5,

    /// <summary>An element id or attribute index was out of range.</summary>
    IndexOutOfBounds = 6,

    /// <summary>scah hit an internal bug. Its state is intact.</summary>
    InternalPanic = 7,

    /// <summary>The HTML, or the text saved from it, is 4 GiB or longer.</summary>
    InputTooLarge = 8,

    /// <summary>An argument was invalid.</summary>
    InvalidArgument = 9,

    /// <summary>A result was too large to export.</summary>
    ExportTooLarge = 10,
}

/// <summary>A call into scah failed.</summary>
public sealed class ScahException : Exception
{
    internal ScahException(ScahStatus status, string message)
        : base(message) => Status = status;

    /// <summary>Why the call failed. May be a value newer than this enum.</summary>
    public ScahStatus Status { get; }

    /// <summary>Build the exception for a failed call, freeing its error.</summary>
    internal static ScahException FromNative(int status, nint error)
    {
        string message;
        try
        {
            message = error == nint.Zero
                ? $"scah call failed with status {status}"
                : Native.scah_error_message(error).Decode();
        }
        finally
        {
            Native.scah_error_free(error);
        }
        return new ScahException((ScahStatus)status, message);
    }
}
