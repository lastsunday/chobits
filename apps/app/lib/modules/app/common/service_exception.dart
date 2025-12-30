class ServiceException implements Exception {
  ServiceException(this.serviceMessage);

  String serviceMessage;
}
