package io.github.zacharyvmm.scah;

/**
 * What to keep for each element a query section matches.
 *
 * <p>The tag name is always kept. Start from a preset and adjust it with the {@code with}
 * methods, such as {@code Save.nameOnly().withText(true)}. Instances are immutable.
 */
public final class Save {
    private static final int INNER_HTML = 1;
    private static final int RAW_TEXT = 2;
    private static final int TEXT = 4;
    private static final int ATTRIBUTES = 8;

    private final int flags;

    private Save(int flags) {
        this.flags = flags;
    }

    /** Keep everything: inner HTML, raw text, text, and attributes. */
    public static Save all() {
        return new Save(INNER_HTML | RAW_TEXT | TEXT | ATTRIBUTES);
    }

    /** Keep attributes, but neither inner HTML nor text. */
    public static Save none() {
        return new Save(ATTRIBUTES);
    }

    /** Keep only the tag name. */
    public static Save nameOnly() {
        return new Save(0);
    }

    /** Keep the inner HTML and attributes. */
    public static Save onlyInnerHtml() {
        return new Save(INNER_HTML | ATTRIBUTES);
    }

    /** Keep the raw text and attributes. */
    public static Save onlyRawText() {
        return new Save(RAW_TEXT | ATTRIBUTES);
    }

    /** Keep the text and attributes. */
    public static Save onlyText() {
        return new Save(TEXT | ATTRIBUTES);
    }

    /** Whether the HTML between the opening and closing tags is kept. */
    public boolean innerHtml() {
        return (flags & INNER_HTML) != 0;
    }

    /** Whether the descendant text is kept exactly as written in the source. */
    public boolean rawText() {
        return (flags & RAW_TEXT) != 0;
    }

    /** Whether the descendant text is kept with entities decoded and whitespace collapsed. */
    public boolean text() {
        return (flags & TEXT) != 0;
    }

    /** Whether the {@code id}, {@code class}, and other attributes are kept. */
    public boolean attributes() {
        return (flags & ATTRIBUTES) != 0;
    }

    /** A copy that keeps the inner HTML when {@code keep} is true. */
    public Save withInnerHtml(boolean keep) {
        return with(INNER_HTML, keep);
    }

    /** A copy that keeps the raw text when {@code keep} is true. */
    public Save withRawText(boolean keep) {
        return with(RAW_TEXT, keep);
    }

    /** A copy that keeps the text when {@code keep} is true. */
    public Save withText(boolean keep) {
        return with(TEXT, keep);
    }

    /** A copy that keeps the attributes when {@code keep} is true. */
    public Save withAttributes(boolean keep) {
        return with(ATTRIBUTES, keep);
    }

    private Save with(int flag, boolean keep) {
        return new Save(keep ? flags | flag : flags & ~flag);
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof Save && ((Save) other).flags == flags;
    }

    @Override
    public int hashCode() {
        return flags;
    }

    @Override
    public String toString() {
        return "Save[innerHtml=" + innerHtml() + ", rawText=" + rawText() + ", text=" + text()
                + ", attributes=" + attributes() + "]";
    }
}
