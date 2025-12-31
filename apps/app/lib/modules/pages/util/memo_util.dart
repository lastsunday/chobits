import 'package:app/l10n/app_localizations.dart';
import 'package:flutter/material.dart';
import 'package:app/modules/app/model/memo_model.dart';

class MemoUtil {
  static String getLableByDisplayMode(
      DisplayMode displayMode, BuildContext context) {
    switch (displayMode) {
      case DisplayMode.auto:
        return AppLocalizations.of(context)!.displayModeAuto;
      case DisplayMode.text:
        return AppLocalizations.of(context)!.displayModeText;
      case DisplayMode.image:
        return AppLocalizations.of(context)!.displayModeImage;
      default:
        throw Exception("unkonw display mode = ${displayMode.name}");
    }
  }

  static IconData getIconByDisplayMode(DisplayMode displayMode) {
    switch (displayMode) {
      case DisplayMode.auto:
        return Icons.remove_red_eye;
      case DisplayMode.text:
        return Icons.text_fields;
      case DisplayMode.image:
        return Icons.image;
      default:
        throw Exception("unkonw display mode = ${displayMode.name}");
    }
  }
}
