import 'package:flutter/material.dart';

class CommonAppBar extends AppBar {
  CommonAppBar(
      {super.key,
      Widget? title,
      Widget? leading,
      bool showLeading = false,
      bool automaticallyImplyLeading = true,
      super.centerTitle = true,
      super.titleTextStyle = const TextStyle(color: Color(0xFF171321), fontSize: 16, fontWeight: FontWeight.w600),
      super.backgroundColor,
      super.elevation = 0,
      super.actions})
      : super(title: title, leading: showLeading ? (leading ?? const BackButton(color: Colors.black)) : null, automaticallyImplyLeading: automaticallyImplyLeading);
}
