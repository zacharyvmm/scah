package io.github.zacharyvmm.scah;

import java.util.AbstractList;
import java.util.RandomAccess;

/** The elements one selector matched, created as they are read. */
final class ElementList extends AbstractList<Element> implements RandomAccess {
    private final Store store;
    private final long[] ids;

    ElementList(Store store, long[] ids) {
        this.store = store;
        this.ids = ids;
    }

    @Override
    public Element get(int index) {
        return new Element(store, ids[index]);
    }

    @Override
    public int size() {
        return ids.length;
    }
}
