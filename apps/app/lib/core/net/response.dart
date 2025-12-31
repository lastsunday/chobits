class Response<T> {
  final T _data;

  static Response<T> of<T>(T t) {
    return Response(t);
  }

  Response(this._data);

  T body() {
    return _data;
  }
}
