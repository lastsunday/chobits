import 'package:json_annotation/json_annotation.dart';

part 'memo_entity.g.dart';

@JsonSerializable()
class MemoEntity {
  static const tableName = "memo";

  MemoEntity({this.id, this.content, this.datetime, this.displaymode});

  String? id;
  String? content;
  String? datetime;
  int? displaymode;
  String? updatedatetime;

  factory MemoEntity.fromJson(Map<String, dynamic> json) =>
      _$MemoEntityFromJson(json);

  Map<String, dynamic> toJson() => _$MemoEntityToJson(this);
}
