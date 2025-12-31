import 'package:json_annotation/json_annotation.dart';

part 'memo_search_bo.g.dart';

@JsonSerializable()
class MemoSearchBo {
  MemoSearchBo(this.content, this.displaymode);

  String? content = "";
  int? displaymode = 0;

  factory MemoSearchBo.fromJson(Map<String, dynamic> json) =>
      _$MemoSearchBoFromJson(json);

  Map<String, dynamic> toJson() => _$MemoSearchBoToJson(this);
}
