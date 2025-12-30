extension IterableExtension<E> on Iterable<E> {
  E reduceOr(E Function(E value, E element) combine, E defaultValue) {
    if (isEmpty) return defaultValue;
    return reduce(combine);
  }

  E firstOr(E other) {
    if (isEmpty) return other;
    return first;
  }

  E? get firstNullable {
    if (isEmpty) return null;
    return first;
  }

  bool allMatch(bool Function(E element) predicate) {
    for (final element in this) {
      if (!predicate(element)) return false;
    }
    return true;
  }
}
