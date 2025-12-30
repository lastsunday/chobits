class Task {
  Task({required this.name, required this.offset, required this.callback});

  String name;
  /*milliseconds */
  int offset;
  DateTime? previousExcuteTime;
  bool running = false;
  Function(Task task) callback;
}
