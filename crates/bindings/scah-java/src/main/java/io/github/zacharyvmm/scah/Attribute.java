package io.github.zacharyvmm.scah;

import java.util.Objects;
import java.util.Optional;

/** An attribute of an element, as written in the source. */
public final class Attribute {
    private final String name;
    private final String value;

    Attribute(String name, String value) {
        this.name = name;
        this.value = value;
    }

    /** The attribute's name. */
    public String name() {
        return name;
    }

    /** The attribute's value, or empty for an attribute without one, as in {@code <input disabled>}. */
    public Optional<String> value() {
        return Optional.ofNullable(value);
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof Attribute)) {
            return false;
        }
        Attribute attribute = (Attribute) other;
        return name.equals(attribute.name) && Objects.equals(value, attribute.value);
    }

    @Override
    public int hashCode() {
        return Objects.hash(name, value);
    }

    @Override
    public String toString() {
        return value == null ? name : name + "=\"" + value + "\"";
    }
}
